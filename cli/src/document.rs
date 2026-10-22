use std::io;
use std::path::{Path, PathBuf};

use spectraplot::scan::Scan;

use crate::files;

/// The scan on screen and where it lives on disk.
#[derive(Default)]
pub struct Document {
    pub scan: Scan,
    pub path: Option<PathBuf>,
    pub export_path: Option<PathBuf>,
    pub dirty: bool,
    pub stem: Option<String>,
}

impl Document {
    /// Loads a document from disk.
    pub fn open(path: &Path) -> io::Result<Self> {
        let scan = files::open(path)?;
        Ok(Self {
            scan,
            path: Some(path.to_path_buf()),
            ..Self::default()
        })
    }

    /// Saves to `path` and remembers it for the next plain Save.
    pub fn save(&mut self, path: &Path) -> io::Result<()> {
        files::save(path, &self.scan)?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// Exports to `path` and remembers it for the next plain Export.
    pub fn export(&mut self, path: &Path) -> io::Result<()> {
        files::export(path, &self.scan)?;
        self.export_path = Some(path.to_path_buf());
        Ok(())
    }

    /// A new, empty document for a scan, named after its plan and start time.
    pub fn for_scan(stem: String) -> Self {
        Self {
            stem: Some(stem),
            ..Self::default()
        }
    }

    /// The file name to suggest in save and export dialogs, without extension.
    pub fn suggested_stem(&self) -> String {
        let from_path = self.path.as_deref().and_then(Path::file_stem);
        match (from_path, &self.stem) {
            (Some(stem), _) => stem.to_string_lossy().into_owned(),
            (None, Some(stem)) => stem.clone(),
            (None, None) => "scan".into(),
        }
    }

    /// File name plus an unsaved marker, for display.
    pub fn title(&self) -> String {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or("unsaved scan".into(), |n| n.to_string_lossy().into_owned());
        if self.dirty {
            format!("{name} *")
        } else {
            name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title() {
        let saved = |dirty| Document {
            path: Some("/data/neon.dat".into()),
            dirty,
            ..Document::default()
        };
        let cases = [
            ("new", Document::default(), "unsaved scan"),
            (
                "new with data",
                Document {
                    dirty: true,
                    ..Document::default()
                },
                "unsaved scan *",
            ),
            ("saved", saved(false), "neon.dat"),
            ("changed", saved(true), "neon.dat *"),
        ];
        for (name, doc, want) in cases {
            assert_eq!(doc.title(), want, "{name}");
        }
    }

    #[test]
    fn suggested_stem() {
        let cases = [
            ("new", Document::default(), "scan"),
            (
                "fresh scan",
                Document::for_scan("Ne_green_20261022-0959".into()),
                "Ne_green_20261022-0959",
            ),
            (
                "saved scan",
                Document {
                    path: Some("/data/neon.dat".into()),
                    ..Document::for_scan("x".into())
                },
                "neon",
            ),
        ];
        for (name, doc, want) in cases {
            assert_eq!(doc.suggested_stem(), want, "{name}");
        }
    }
}
