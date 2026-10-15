use std::fs::File;
use std::io::{BufRead, ErrorKind, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serialport::SerialPort;
use specplot::event::Event;
use specplot::sample::Sample;

const BAUD: u32 = 115_200;
const READ_TIMEOUT: Duration = Duration::from_secs(1);
const LINE_END: &str = "\r";

/// Opens the spectractrl serial port.
pub fn open_port(path: &str) -> serialport::Result<Box<dyn SerialPort>> {
    serialport::new(path, BAUD).timeout(READ_TIMEOUT).open()
}

/// Sends one command line, terminated the way the REPL expects.
pub fn send_line(port: &mut dyn SerialPort, line: &str) -> std::io::Result<()> {
    port.write_all(line.as_bytes())?;
    port.write_all(LINE_END.as_bytes())?;
    port.flush()
}

/// Reads device output on a background thread and hands out classified events.
/// Samples are also appended to `csv`, if given.
pub fn spawn_reader(input: impl BufRead + Send + 'static, csv: Option<File>) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || read_events(input, &tx, csv));
    rx
}

fn read_events(mut input: impl BufRead, tx: &Sender<Event>, mut csv: Option<File>) {
    let mut line = String::new();
    loop {
        match input.read_line(&mut line) {
            Ok(0) => return,
            Ok(_) => {
                if !forward(&line, tx, csv.as_mut()) {
                    return;
                }
                line.clear();
            }
            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
            Err(e) => {
                eprintln!("read failed: {e}");
                return;
            }
        }
    }
}

fn forward(line: &str, tx: &Sender<Event>, csv: Option<&mut File>) -> bool {
    let Some(event) = Event::parse(line) else {
        return true;
    };
    if let (Event::Sample(sample), Some(csv)) = (&event, csv) {
        append_csv(csv, sample);
    }
    tx.send(event).is_ok()
}

fn append_csv(csv: &mut File, sample: &Sample) {
    if let Err(e) = writeln!(csv, "{}", sample.to_csv()) {
        eprintln!("csv write failed: {e}");
    }
}
