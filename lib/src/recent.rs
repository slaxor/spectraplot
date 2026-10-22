use std::path::{Path, PathBuf};

const LIMIT: usize = 10;

/// Most recently used files, newest first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recent {
    pub paths: Vec<PathBuf>,
}

impl Recent {
    /// Reads the one-path-per-line state file.
    pub fn parse(text: &str) -> Self {
        let paths = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(PathBuf::from)
            .take(LIMIT);
        Self {
            paths: paths.collect(),
        }
    }

    /// Renders the state file.
    pub fn to_text(&self) -> String {
        self.paths
            .iter()
            .map(|p| format!("{}\n", p.display()))
            .collect()
    }

    /// Moves `path` to the front, dropping duplicates and the oldest beyond the limit.
    pub fn push(&mut self, path: &Path) {
        self.paths.retain(|p| p != path);
        self.paths.insert(0, path.to_path_buf());
        self.paths.truncate(LIMIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recent(paths: &[&str]) -> Recent {
        Recent {
            paths: paths.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn parse() {
        let many: String = (0..12).map(|i| format!("/{i}.dat\n")).collect();
        let first_ten: Vec<String> = (0..10).map(|i| format!("/{i}.dat")).collect();
        let first_ten: Vec<&str> = first_ten.iter().map(String::as_str).collect();
        let cases = [
            ("two", "/a.dat\n/b.dat\n", recent(&["/a.dat", "/b.dat"])),
            ("blank lines", "\n/a.dat\n  \n", recent(&["/a.dat"])),
            ("capped", many.as_str(), recent(&first_ten)),
            ("empty", "", Recent::default()),
        ];
        for (name, text, want) in cases {
            assert_eq!(Recent::parse(text), want, "{name}");
        }
    }

    #[test]
    fn push() {
        let full: Vec<String> = (0..LIMIT).map(|i| format!("/{i}")).collect();
        let full: Vec<&str> = full.iter().map(String::as_str).collect();
        let cases = [
            ("into empty", recent(&[]), "/a", recent(&["/a"])),
            ("new in front", recent(&["/a"]), "/b", recent(&["/b", "/a"])),
            (
                "existing moves up",
                recent(&["/a", "/b"]),
                "/b",
                recent(&["/b", "/a"]),
            ),
            (
                "oldest dropped",
                recent(&full),
                "/new",
                recent(&[&["/new"], &full[..LIMIT - 1]].concat()),
            ),
        ];
        for (name, mut recent, path, want) in cases {
            recent.push(Path::new(path));
            assert_eq!(recent, want, "{name}");
        }
    }

    #[test]
    fn roundtrip() {
        let recent = recent(&["/a.dat", "/b.dat"]);
        assert_eq!(Recent::parse(&recent.to_text()), recent);
    }
}
