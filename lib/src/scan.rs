use crate::event::Event;
use crate::sample::Sample;

const NOTE_PREFIX: &str = "#>";
const MARK_PREFIX: &str = "#@";

/// A comment on one measurement point, found by its wavelength.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub nm: f64,
    pub text: String,
}

/// A recorded scan: the device's description line, free notes, marks and samples.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scan {
    pub header: Option<String>,
    pub notes: String,
    pub marks: Vec<Mark>,
    pub samples: Vec<Sample>,
}

impl Scan {
    /// Reads a `.dat` file or a raw device log; unrecognised lines are skipped.
    pub fn parse(text: &str) -> Self {
        let mut scan = Self::default();
        let mut notes = Vec::new();
        for line in text.lines() {
            if let Some(note) = line.strip_prefix(NOTE_PREFIX) {
                notes.push(note.strip_prefix(' ').unwrap_or(note));
            } else if let Some(mark) = line.strip_prefix(MARK_PREFIX) {
                scan.parse_mark(mark);
            } else if let Some(event) = Event::parse(line) {
                scan.apply(event);
            }
        }
        scan.notes = notes.join("\n");
        scan
    }

    fn parse_mark(&mut self, text: &str) {
        let (nm, text) = text
            .trim_start()
            .split_once(' ')
            .unwrap_or((text.trim(), ""));
        if let Ok(nm) = nm.parse() {
            self.set_mark(nm, text);
        }
    }

    /// The wavelength of the sample closest to `nm`.
    pub fn nearest_nm(&self, nm: f64) -> Option<f64> {
        let distance = |s: &&Sample| (s.nm - nm).abs();
        let nearest = self
            .samples
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)));
        nearest.map(|s| s.nm)
    }

    /// Keeps the marks ordered by wavelength, as `set_mark` does.
    pub fn sort_marks(&mut self) {
        self.marks.sort_by(|a, b| a.nm.total_cmp(&b.nm));
    }

    /// Index of the mark at `nm`, if there is one.
    pub fn mark_index(&self, nm: f64) -> Option<usize> {
        self.marks.iter().position(|m| (m.nm - nm).abs() < 1e-6)
    }

    /// Sets the comment on the point at `nm`, adding the mark if needed; marks stay sorted.
    /// Line breaks become spaces, as a mark is a single line in the file.
    pub fn set_mark(&mut self, nm: f64, text: &str) {
        let text = text.replace(['\r', '\n'], " ");
        match self.mark_index(nm) {
            Some(index) => self.marks[index].text = text,
            None => {
                self.marks.push(Mark { nm, text });
                self.sort_marks();
            }
        }
    }

    /// Takes over a header or sample; returns true if the scan changed.
    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Header(header) if self.header.is_none() => self.header = Some(header),
            Event::Sample(sample) => self.samples.push(sample),
            _ => return false,
        }
        true
    }

    /// The native file format: `# header`, notes as `#> line`, marks as `#@ nm text`,
    /// then `nm,read,...` rows. All of it but the rows is a comment to other tools.
    pub fn to_dat(&self) -> String {
        let header = self.header.iter().map(|h| format!("# {h}"));
        let notes = self
            .notes
            .lines()
            .map(|n| format!("{NOTE_PREFIX} {n}").trim_end().to_string());
        let marks = self
            .marks
            .iter()
            .map(|m| format!("{MARK_PREFIX} {} {}", m.nm, m.text));
        let rows = self.samples.iter().map(Sample::to_csv);
        lines(header.chain(notes).chain(marks).chain(rows))
    }

    /// A plain table for other tools: column names, reads and their mean.
    pub fn to_csv(&self) -> String {
        let reads = self
            .samples
            .iter()
            .map(|s| s.reads.len())
            .max()
            .unwrap_or(0);
        let names = (1..=reads).map(|i| format!("read{i}"));
        let header = std::iter::once("nm".to_string())
            .chain(names)
            .chain(["mean".to_string()]);
        let rows = self
            .samples
            .iter()
            .map(|s| format!("{},{:.1}", s.to_csv(), s.mean()));
        lines(std::iter::once(header.collect::<Vec<_>>().join(",")).chain(rows))
    }
}

impl Scan {
    /// The same table as [`Scan::to_csv`], tab separated, so that a spreadsheet takes
    /// it from the clipboard as columns. No field contains a comma, so swapping is safe.
    pub fn to_tsv(&self) -> String {
        self.to_csv().replace(',', "\t")
    }
}

fn lines(lines: impl Iterator<Item = String>) -> String {
    lines.map(|line| line + "\n").collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAT: &str = "# grating 3, 535.0..545.0 nm\n535,4674\n535.1,3475\n";

    fn scan() -> Scan {
        Scan {
            header: Some("grating 3, 535.0..545.0 nm".into()),
            samples: vec![
                Sample {
                    nm: 535.0,
                    reads: vec![4674],
                },
                Sample {
                    nm: 535.1,
                    reads: vec![3475],
                },
            ],
            ..Scan::default()
        }
    }

    #[test]
    fn parse() {
        let device_log = "scan started\n[scan:out] # grating 3, 535.0..545.0 nm\n[scan:out] 535.0,4674\n[scan:out] 535.1,3475\n[scan:done] 2 steps\n";
        let cases = [
            ("dat file", DAT, scan()),
            ("device log", device_log, scan()),
            (
                "no header",
                "535.0,4674\n",
                Scan {
                    header: None,
                    samples: vec![Sample {
                        nm: 535.0,
                        reads: vec![4674],
                    }],
                    ..Scan::default()
                },
            ),
            ("empty", "", Scan::default()),
        ];
        for (name, text, want) in cases {
            assert_eq!(Scan::parse(text), want, "{name}");
        }
    }

    #[test]
    fn apply() {
        let sample = Sample {
            nm: 1.0,
            reads: vec![1],
        };
        let cases = [
            (
                "first header",
                Scan::default(),
                Event::Header("h".into()),
                true,
            ),
            ("second header", scan(), Event::Header("h".into()), false),
            ("sample", scan(), Event::Sample(sample), true),
            ("done", scan(), Event::ScanDone, false),
            ("other", scan(), Event::Other("x".into()), false),
        ];
        for (name, mut scan, event, want) in cases {
            assert_eq!(scan.apply(event), want, "{name}");
        }
    }

    #[test]
    fn to_dat() {
        let cases = [("with header", scan(), DAT), ("empty", Scan::default(), "")];
        for (name, scan, want) in cases {
            assert_eq!(scan.to_dat(), want, "{name}");
            assert_eq!(Scan::parse(&scan.to_dat()), scan, "{name} roundtrip");
        }
    }

    #[test]
    fn to_csv() {
        let three = Scan {
            header: None,
            samples: vec![Sample {
                nm: 700.0,
                reads: vec![1, 2, 6],
            }],
            ..Scan::default()
        };
        let cases = [
            (
                "one read",
                scan(),
                "nm,read1,mean\n535,4674,4674.0\n535.1,3475,3475.0\n",
            ),
            (
                "three reads",
                three,
                "nm,read1,read2,read3,mean\n700,1,2,6,3.0\n",
            ),
            ("empty", Scan::default(), "nm,mean\n"),
        ];
        for (name, scan, want) in cases {
            assert_eq!(scan.to_csv(), want, "{name}");
        }
    }

    #[test]
    fn to_tsv() {
        let three = Scan {
            header: None,
            samples: vec![Sample {
                nm: 700.0,
                reads: vec![1, 2, 6],
            }],
            ..Scan::default()
        };
        let cases = [
            (
                "one read",
                scan(),
                "nm\tread1\tmean\n535\t4674\t4674.0\n535.1\t3475\t3475.0\n",
            ),
            (
                "three reads",
                three,
                "nm\tread1\tread2\tread3\tmean\n700\t1\t2\t6\t3.0\n",
            ),
            ("empty", Scan::default(), "nm\tmean\n"),
        ];
        for (name, scan, want) in cases {
            assert_eq!(scan.to_tsv(), want, "{name}");
        }
    }

    fn annotated() -> Scan {
        Scan {
            notes: "Ne lamp, fine step\n\nsecond paragraph".into(),
            marks: vec![
                Mark {
                    nm: 535.0,
                    text: "start of the line".into(),
                },
                Mark {
                    nm: 535.1,
                    text: String::new(),
                },
            ],
            ..scan()
        }
    }

    #[test]
    fn annotations_survive_the_file() {
        let text = annotated().to_dat();
        assert_eq!(
            text,
            "# grating 3, 535.0..545.0 nm\n#> Ne lamp, fine step\n#>\n#> second paragraph\n\
             #@ 535 start of the line\n#@ 535.1 \n535,4674\n535.1,3475\n"
        );
        assert_eq!(Scan::parse(&text), annotated());
    }

    #[test]
    fn annotations_do_not_disturb_other_readers() {
        let events = annotated().to_dat();
        let header_lines = events.lines().filter(|l| l.starts_with("# ")).count();
        assert_eq!(header_lines, 1);
        assert_eq!(Scan::parse(DAT), scan());
    }

    #[test]
    fn set_mark() {
        let cases = [
            (
                "adds sorted",
                vec![(536.0, "b"), (535.0, "a")],
                vec![(535.0, "a"), (536.0, "b")],
            ),
            (
                "replaces",
                vec![(535.0, "a"), (535.0, "c")],
                vec![(535.0, "c")],
            ),
            ("one line", vec![(535.0, "a\nb")], vec![(535.0, "a b")]),
        ];
        for (name, calls, want) in cases {
            let mut scan = Scan::default();
            for (nm, text) in calls {
                scan.set_mark(nm, text);
            }
            let got: Vec<_> = scan.marks.iter().map(|m| (m.nm, m.text.as_str())).collect();
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn nearest_nm() {
        let cases = [
            ("between, closer to the first", 535.04, Some(535.0)),
            ("between, closer to the second", 535.06, Some(535.1)),
            ("beyond the end", 900.0, Some(535.1)),
        ];
        for (name, nm, want) in cases {
            assert_eq!(scan().nearest_nm(nm), want, "{name}");
        }
        assert_eq!(Scan::default().nearest_nm(1.0), None);
    }
}
