use std::io::{BufRead, BufReader, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use spectraplot::event::Event;

const BAUD: u32 = 115_200;
const READ_TIMEOUT: Duration = Duration::from_secs(1);
const LINE_END: &str = "\r";

/// An open spectractrl port with its reader thread; dropping it stops the reader.
pub struct Connection {
    pub path: String,
    pub events: Receiver<Event>,
    port: Box<dyn SerialPort>,
    stop: Arc<AtomicBool>,
}

impl Connection {
    /// Opens `path` and starts reading; DTR is raised where the port supports it,
    /// as USB CDC devices only send once it is set.
    pub fn open(path: &str) -> serialport::Result<Self> {
        let mut port = serialport::new(path, BAUD).timeout(READ_TIMEOUT).open()?;
        if let Err(e) = port.write_data_terminal_ready(true) {
            eprintln!("cannot raise DTR on {path}: {e}");
        }
        let stop = Arc::new(AtomicBool::new(false));
        let reader = BufReader::new(port.try_clone()?);
        let events = spawn_reader(reader, Arc::clone(&stop));
        Ok(Self {
            path: path.to_string(),
            events,
            port,
            stop,
        })
    }

    /// Sends one command line, terminated the way the REPL expects.
    pub fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        self.port.write_all(line.as_bytes())?;
        self.port.write_all(LINE_END.as_bytes())?;
        self.port.flush()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A port offered in the Port menu.
#[derive(Debug, PartialEq)]
pub struct PortChoice {
    pub path: String,
    pub label: String,
}

/// Lists the serial ports present right now, sorted by path.
pub fn list_ports() -> Vec<PortChoice> {
    let mut ports = match serialport::available_ports() {
        Ok(ports) => ports,
        Err(e) => {
            eprintln!("cannot list serial ports: {e}");
            Vec::new()
        }
    };
    ports.sort_by(|a, b| a.port_name.cmp(&b.port_name));
    ports.iter().map(choice).collect()
}

fn choice(info: &SerialPortInfo) -> PortChoice {
    PortChoice {
        path: info.port_name.clone(),
        label: label(info),
    }
}

fn label(info: &SerialPortInfo) -> String {
    let SerialPortType::UsbPort(usb) = &info.port_type else {
        return info.port_name.clone();
    };
    let names: Vec<&str> = [&usb.manufacturer, &usb.product]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();
    if names.is_empty() {
        format!("{}  ({:04x}:{:04x})", info.port_name, usb.vid, usb.pid)
    } else {
        format!("{}  {}", info.port_name, names.join(" "))
    }
}

fn spawn_reader(input: impl BufRead + Send + 'static, stop: Arc<AtomicBool>) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || read_events(input, &tx, &stop));
    rx
}

fn read_events(mut input: impl BufRead, tx: &Sender<Event>, stop: &AtomicBool) {
    let mut line = String::new();
    while !stop.load(Ordering::Relaxed) {
        match input.read_line(&mut line) {
            Ok(0) => return report(tx, "device closed the connection".into()),
            Ok(_) => {
                let event = Event::parse(&line);
                line.clear();
                if let Some(event) = event
                    && tx.send(event).is_err()
                {
                    return;
                }
            }
            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
            Err(e) => return report(tx, format!("read failed: {e}")),
        }
    }
}

fn report(tx: &Sender<Event>, message: String) {
    tx.send(Event::Other(message)).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::UsbPortInfo;
    use spectraplot::sample::Sample;
    use std::io::Cursor;

    #[test]
    fn reader_forwards_events_until_eof() {
        let input =
            "scan started\n\n[scan:out] # grating 2\n[scan:out] 700.0,1441\n[scan:done] 1 steps\n";
        let stop = Arc::new(AtomicBool::new(false));
        let events: Vec<Event> = spawn_reader(Cursor::new(input.to_string()), stop)
            .iter()
            .collect();
        let want = vec![
            Event::Other("scan started".into()),
            Event::Header("grating 2".into()),
            Event::Sample(Sample {
                nm: 700.0,
                reads: vec![1441],
            }),
            Event::ScanDone,
            Event::Other("device closed the connection".into()),
        ];
        assert_eq!(events, want);
    }

    #[test]
    fn reader_stops_when_asked() {
        let stop = Arc::new(AtomicBool::new(true));
        let events: Vec<Event> = spawn_reader(Cursor::new("700.0,1\n".to_string()), stop)
            .iter()
            .collect();
        assert_eq!(events, vec![]);
    }

    fn usb(manufacturer: Option<&str>, product: Option<&str>) -> SerialPortType {
        SerialPortType::UsbPort(UsbPortInfo {
            vid: 0x2e8a,
            pid: 0x000a,
            serial_number: None,
            manufacturer: manufacturer.map(Into::into),
            product: product.map(Into::into),
        })
    }

    #[test]
    fn label() {
        let cases = [
            (
                "usb with names",
                usb(Some("slaxor"), Some("spectractrl")),
                "/dev/ttyACM0  slaxor spectractrl",
            ),
            (
                "usb product only",
                usb(None, Some("spectractrl")),
                "/dev/ttyACM0  spectractrl",
            ),
            (
                "usb without names",
                usb(None, None),
                "/dev/ttyACM0  (2e8a:000a)",
            ),
            ("not usb", SerialPortType::PciPort, "/dev/ttyACM0"),
        ];
        for (name, port_type, want) in cases {
            let info = SerialPortInfo {
                port_name: "/dev/ttyACM0".into(),
                port_type,
            };
            assert_eq!(super::label(&info), want, "{name}");
        }
    }
}
