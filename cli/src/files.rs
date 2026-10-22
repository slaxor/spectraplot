use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use spectraplot::recent::Recent;
use spectraplot::scan::Scan;
use spectraplot::svg;

const DAT: &str = "dat";
const CSV: &str = "csv";
const SVG: &str = "svg";

/// Loads a `.dat` file or a raw device log.
pub fn open(path: &Path) -> io::Result<Scan> {
    fs::read_to_string(path).map(|text| Scan::parse(&text))
}

/// Writes the scan in the native `.dat` format.
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
        .add_filter("scans", &[DAT, CSV, "log", "txt"])
        .add_filter("all files", &["*"])
        .pick_file()
}

/// Asks where to save, suggesting a `.dat` file.
pub fn pick_save() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("scan", &[DAT])
        .set_file_name("scan.dat")
        .save_file()
}

/// Asks where to export; the chosen extension selects the format.
pub fn pick_export() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("table", &[CSV])
        .add_filter("plot", &[SVG])
        .set_file_name("scan.csv")
        .save_file()
}

/// Loads the recent-files list; a missing file means no history yet.
pub fn load_recent() -> Recent {
    match fs::read_to_string(recent_path()) {
        Ok(text) => Recent::parse(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Recent::default(),
        Err(e) => {
            eprintln!("cannot read recent files: {e}");
            Recent::default()
        }
    }
}

/// Stores the recent-files list.
pub fn store_recent(recent: &Recent) -> io::Result<()> {
    let path = recent_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, recent.to_text())
}

fn recent_path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_default();
    state.join("spectraplot").join("recent")
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
