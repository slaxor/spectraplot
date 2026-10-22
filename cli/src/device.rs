use std::io::{BufRead, ErrorKind};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serialport::SerialPort;
use spectraplot::event::Event;

const BAUD: u32 = 115_200;
const READ_TIMEOUT: Duration = Duration::from_secs(1);
const LINE_END: &str = "\r";

/// Opens the spectractrl serial port with DTR raised, as USB CDC devices expect.
pub fn open_port(path: &str) -> serialport::Result<Box<dyn SerialPort>> {
    let mut port = serialport::new(path, BAUD).timeout(READ_TIMEOUT).open()?;
    port.write_data_terminal_ready(true)?;
    Ok(port)
}

/// Sends one command line, terminated the way the REPL expects.
pub fn send_line(port: &mut dyn SerialPort, line: &str) -> std::io::Result<()> {
    port.write_all(line.as_bytes())?;
    port.write_all(LINE_END.as_bytes())?;
    port.flush()
}

/// Reads device output on a background thread and hands out classified events.
pub fn spawn_reader(input: impl BufRead + Send + 'static) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || read_events(input, &tx));
    rx
}

fn read_events(mut input: impl BufRead, tx: &Sender<Event>) {
    let mut line = String::new();
    loop {
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
    use spectraplot::sample::Sample;
    use std::io::Cursor;

    #[test]
    fn reader_forwards_events_until_eof() {
        let input =
            "scan started\n\n[scan:out] # grating 2\n[scan:out] 700.0,1441\n[scan:done] 1 steps\n";
        let events: Vec<Event> = spawn_reader(Cursor::new(input.to_string()))
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
}
