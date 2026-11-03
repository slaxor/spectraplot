use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serialport::{ClearBuffer, SerialPort, SerialPortInfo, SerialPortType};
use spectraplot::event::{self, Event};

const BAUD: u32 = 115_200;
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const LINE_END: &str = "\r";

/// One received line as a terminal would show it, and what it means.
#[derive(Debug, PartialEq)]
pub struct Message {
    pub text: String,
    pub event: Event,
}

/// An open spectractrl port. A reader thread turns incoming lines into messages,
/// a writer thread owns the port and sends lines; the UI never touches the port,
/// so a device that stops listening can never block it.
pub struct Connection {
    pub path: String,
    pub messages: Receiver<Message>,
    lines: Sender<String>,
    stop: Arc<AtomicBool>,
}

impl Connection {
    /// Opens `path` and starts both threads; DTR is raised where the port supports it,
    /// as USB CDC devices only send once it is set. `wake` is called after every
    /// message, so the UI can redraw right away instead of polling.
    pub fn open(path: &str, wake: impl Fn() + Send + Sync + 'static) -> serialport::Result<Self> {
        let mut port = serialport::new(path, BAUD).timeout(IO_TIMEOUT).open()?;
        if let Err(e) = port.write_data_terminal_ready(true) {
            eprintln!("cannot raise DTR on {path}: {e}");
        }
        let (tx, messages) = mpsc::channel();
        let outbox = Outbox {
            tx,
            wake: Arc::new(wake),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let reader = BufReader::new(port.try_clone()?);
        spawn_reader(reader, Arc::clone(&stop), outbox.clone());
        let lines = spawn_writer(port, outbox);
        Ok(Self {
            path: path.to_string(),
            messages,
            lines,
            stop,
        })
    }

    /// Queues one command line; a failure to send comes back as a message.
    pub fn send_line(&self, line: &str) {
        self.lines.send(line.to_string()).ok();
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Where both threads deliver their messages, waking the UI each time.
#[derive(Clone)]
struct Outbox {
    tx: Sender<Message>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Outbox {
    fn send(&self, message: Message) -> bool {
        let sent = self.tx.send(message).is_ok();
        (self.wake)();
        sent
    }

    fn report(&self, text: String, event: Event) {
        self.send(Message { text, event });
    }
}

/// A port offered in the Port menu.
#[derive(Debug, PartialEq)]
pub struct PortChoice {
    pub path: String,
    pub label: String,
}

/// Lists the USB serial ports present right now, sorted by path.
pub fn list_ports() -> Vec<PortChoice> {
    let mut ports = match serialport::available_ports() {
        Ok(ports) => ports,
        Err(e) => {
            eprintln!("cannot list serial ports: {e}");
            Vec::new()
        }
    };
    ports.retain(is_usb);
    ports.sort_by(|a, b| a.port_name.cmp(&b.port_name));
    ports.iter().map(choice).collect()
}

fn is_usb(info: &SerialPortInfo) -> bool {
    matches!(info.port_type, SerialPortType::UsbPort(_))
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

fn spawn_reader(input: impl BufRead + Send + 'static, stop: Arc<AtomicBool>, outbox: Outbox) {
    std::thread::spawn(move || read_events(input, &outbox, &stop));
}

fn read_events(mut input: impl BufRead, outbox: &Outbox, stop: &AtomicBool) {
    let mut line = String::new();
    while !stop.load(Ordering::Relaxed) {
        match input.read_line(&mut line) {
            Ok(0) => return disconnected(outbox, "device closed the connection".into()),
            Ok(_) => {
                let text = event::clean(&line);
                line.clear();
                if let Some(event) = Event::parse(&text)
                    && !outbox.send(Message { text, event })
                {
                    return;
                }
            }
            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
            Err(e) => return disconnected(outbox, format!("read failed: {e}")),
        }
    }
}

fn disconnected(outbox: &Outbox, reason: String) {
    outbox.report(reason.clone(), Event::Disconnected(reason));
}

/// Sends queued lines until the connection is dropped, then discards whatever the
/// device has not taken yet: closing a tty with pending output waits for it to drain.
fn spawn_writer(mut port: Box<dyn SerialPort>, outbox: Outbox) -> Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in rx {
            if let Err(e) = write_line(&mut port, &line) {
                let text = format!("send failed, device not listening? {e}");
                outbox.report(text.clone(), Event::Other(text));
            }
        }
        if let Err(e) = port.clear(ClearBuffer::All) {
            eprintln!("cannot discard pending output: {e}");
        }
    });
    tx
}

/// Writes one line with the REPL's line end; deliberately no flush, which would
/// wait without limit for a device that does not read.
fn write_line(out: &mut impl Write, line: &str) -> std::io::Result<()> {
    out.write_all(format!("{line}{LINE_END}").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::UsbPortInfo;
    use spectraplot::sample::Sample;
    use std::io::Cursor;

    fn message(text: &str, event: Event) -> Message {
        Message {
            text: text.into(),
            event,
        }
    }

    fn read_all(input: &str) -> (Vec<Message>, usize) {
        let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&wakes);
        let (tx, rx) = mpsc::channel();
        let outbox = Outbox {
            tx,
            wake: Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }),
        };
        let stop = AtomicBool::new(false);
        read_events(Cursor::new(input.to_string()), &outbox, &stop);
        drop(outbox);
        (rx.iter().collect(), wakes.load(Ordering::Relaxed))
    }

    #[test]
    fn reader_forwards_messages_until_eof() {
        let input = "scan started\n\n> \r\x1b[K[scan:out] 700.0,1441\r\n[scan:done] 1 steps\n";
        let want = vec![
            message("scan started", Event::Other("scan started".into())),
            message(
                "[scan:out] 700.0,1441",
                Event::Sample(Sample {
                    nm: 700.0,
                    reads: vec![1441],
                }),
            ),
            message("[scan:done] 1 steps", Event::ScanDone),
            message(
                "device closed the connection",
                Event::Disconnected("device closed the connection".into()),
            ),
        ];
        let (messages, wakes) = read_all(input);
        assert_eq!(messages, want);
        assert_eq!(wakes, want.len());
    }

    #[test]
    fn reader_stops_when_asked() {
        let (tx, rx) = mpsc::channel();
        let outbox = Outbox {
            tx,
            wake: Arc::new(|| {}),
        };
        let stop = AtomicBool::new(true);
        read_events(Cursor::new("700.0,1\n".to_string()), &outbox, &stop);
        drop(outbox);
        assert_eq!(rx.iter().count(), 0);
    }

    #[test]
    fn write_line_appends_line_end() {
        let mut out = Vec::new();
        write_line(&mut out, "abort scan").expect("write to vec");
        assert_eq!(out, b"abort scan\r");
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
