use crate::event::Event;
use crate::sample::Sample;

/// A recorded scan: the device's description line and its samples.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scan {
    pub header: Option<String>,
    pub samples: Vec<Sample>,
}

impl Scan {
    /// Reads a `.dat` file or a raw device log; unrecognised lines are skipped.
    pub fn parse(text: &str) -> Self {
        let mut scan = Self::default();
        for event in text.lines().filter_map(Event::parse) {
            scan.apply(event);
        }
        scan
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

    /// The native file format: `# header` followed by `nm,read,...` rows.
    pub fn to_dat(&self) -> String {
        let header = self.header.iter().map(|h| format!("# {h}"));
        let rows = self.samples.iter().map(Sample::to_csv);
        lines(header.chain(rows))
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
}
