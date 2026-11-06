use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use spectraplot::history::History;
use spectraplot::plan::ScanPlan;
use spectraplot::presets::Presets;
use spectraplot::scan::Scan;
use spectraplot::svg;

const SPECTRA: &str = "spectra";
const DAT: &str = "dat";
const CSV: &str = "csv";
const SVG: &str = "svg";
const RECENT: &str = "recent";
const RECENT_LIMIT: usize = 10;
const LAST: &str = "last";
const LEGACY_PLANS: &str = "plans";
const PRESETS: &str = "presets";
const PORT: &str = "port";

/// Loads a `.spectra` or `.dat` file or a raw device log.
pub fn open(path: &Path) -> io::Result<Scan> {
    fs::read_to_string(path).map(|text| Scan::parse(&text))
}

/// Writes the scan in the native format.
pub fn save(path: &Path, scan: &Scan) -> io::Result<()> {
    fs::write(path, scan.to_dat())
}

/// Writes the scan as `.csv` table or `.svg` plot, chosen by the file extension.
pub fn export(path: &Path, scan: &Scan) -> io::Result<()> {
    let content = match extension(path).as_str() {
        CSV => scan.to_csv(),
        SVG => svg::render(scan),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cannot export to .{other}, use .csv or .svg"),
            ));
        }
    };
    fs::write(path, content)
}

/// Asks for a file to open.
pub fn pick_open() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("scans", &[SPECTRA, DAT, CSV, "log", "txt"])
        .add_filter("all files", &["*"])
        .pick_file()
}

/// Asks where to save, suggesting `stem.spectra`; `.dat` holds the same text.
pub fn pick_save(stem: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("scan", &[SPECTRA, DAT])
        .set_file_name(format!("{stem}.{SPECTRA}"))
        .save_file()
}

/// Asks where to export, suggesting `stem.csv`; the chosen extension selects the format.
pub fn pick_export(stem: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("table", &[CSV])
        .add_filter("plot", &[SVG])
        .set_file_name(format!("{stem}.{CSV}"))
        .save_file()
}

/// Loads the recently used files.
pub fn load_recent() -> History<PathBuf> {
    History::parse(&read_state(RECENT), RECENT_LIMIT, |line| {
        (!line.is_empty()).then(|| PathBuf::from(line))
    })
}

/// Stores the recently used files.
pub fn store_recent(recent: &History<PathBuf>) -> io::Result<()> {
    write_state(RECENT, &recent.to_text(|path| path.display().to_string()))
}

/// Loads the scan settings used last; older versions kept them in `plans`.
pub fn load_last() -> Option<ScanPlan> {
    let last = read_state(LAST);
    let text = if last.trim().is_empty() {
        read_state(LEGACY_PLANS)
    } else {
        last
    };
    ScanPlan::first_of(&text)
}

/// Remembers the scan settings used last, replacing the previous ones.
pub fn store_last(plan: &ScanPlan) -> io::Result<()> {
    write_state(LAST, &format!("{}\n", plan.to_line()))
}

/// Loads the named scan settings.
pub fn load_presets() -> Presets {
    Presets::parse(&read_state(PRESETS))
}

/// Stores the named scan settings.
pub fn store_presets(presets: &Presets) -> io::Result<()> {
    write_state(PRESETS, &presets.to_text())
}

/// The serial port used last, if any.
pub fn load_port() -> Option<String> {
    let port = read_state(PORT).trim().to_string();
    (!port.is_empty()).then_some(port)
}

/// Remembers the serial port for the next start.
pub fn store_port(path: &str) -> io::Result<()> {
    write_state(PORT, &format!("{path}\n"))
}

fn read_state(name: &str) -> String {
    match fs::read_to_string(state_dir().join(name)) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            eprintln!("cannot read {name}: {e}");
            String::new()
        }
    }
}

fn write_state(name: &str, text: &str) -> io::Result<()> {
    let dir = state_dir();
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(name), text)
}

fn state_dir() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_default();
    state.join("spectraplot")
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectraplot::sample::Sample;

    fn scan() -> Scan {
        Scan {
            header: Some("grating 3".into()),
            samples: vec![Sample {
                nm: 540.0,
                reads: vec![39560],
            }],
            ..Scan::default()
        }
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("spectraplot-files-test-{name}"))
    }

    #[test]
    fn save_then_open() {
        let path = temp("roundtrip.dat");
        save(&path, &scan()).expect("save");
        assert_eq!(open(&path).expect("open"), scan());
    }

    #[test]
    fn export_by_extension() {
        let cases = [
            ("csv", "scan.csv", Ok("nm,read1,mean")),
            ("svg", "scan.SVG", Ok("<svg")),
            ("unknown", "scan.png", Err(io::ErrorKind::InvalidInput)),
            ("none", "scan", Err(io::ErrorKind::InvalidInput)),
        ];
        for (name, file, want) in cases {
            let path = temp(file);
            let got =
                export(&path, &scan()).map(|()| fs::read_to_string(&path).expect("read back"));
            match want {
                Ok(start) => assert!(got.expect(name).starts_with(start), "{name}"),
                Err(kind) => assert_eq!(got.map_err(|e| e.kind()).err(), Some(kind), "{name}"),
            }
        }
    }
}
