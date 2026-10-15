/// One wavelength step: grating position in nm and the raw PMT reads taken there.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub nm: f64,
    pub reads: Vec<u32>,
}

impl Sample {
    /// Parses a scan line like `[scan:out] 700.1,1433,1346,1388`.
    ///
    /// The `[scan:out]` prefix is optional. Headers, status lines and lines
    /// with a missing or malformed field yield `None`.
    pub fn parse(line: &str) -> Option<Self> {
        let csv = line.trim().trim_start_matches("[scan:out]").trim();
        let mut fields = csv.split(',');
        let nm = fields.next()?.trim().parse().ok()?;
        let reads: Vec<u32> = fields
            .map(|f| f.trim().parse())
            .collect::<Result<_, _>>()
            .ok()?;
        (!reads.is_empty()).then_some(Self { nm, reads })
    }

    /// Arithmetic mean of the reads.
    pub fn mean(&self) -> f64 {
        let sum: f64 = self.reads.iter().copied().map(f64::from).sum();
        sum / self.reads.len() as f64
    }

    /// Renders the sample as `nm,read,read,...` without a trailing newline.
    pub fn to_csv(&self) -> String {
        let reads: Vec<String> = self.reads.iter().map(u32::to_string).collect();
        format!("{},{}", self.nm, reads.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(nm: f64, reads: &[u32]) -> Sample {
        Sample {
            nm,
            reads: reads.to_vec(),
        }
    }

    #[test]
    fn parse() {
        let cases = [
            (
                "prefixed",
                "[scan:out] 700.1,1433,1346,1388",
                Some(sample(700.1, &[1433, 1346, 1388])),
            ),
            ("bare", "700.0,1441", Some(sample(700.0, &[1441]))),
            (
                "padded",
                "  700.2 , 1407 ,1356 ",
                Some(sample(700.2, &[1407, 1356])),
            ),
            ("header", "[scan:out] # grating 2, 700.0..750.0 nm", None),
            ("status", "[scan:done] 501 steps", None),
            ("no reads", "700.0", None),
            ("bad read", "700.0,1441,abc", None),
            ("empty", "", None),
        ];
        for (name, line, want) in cases {
            assert_eq!(Sample::parse(line), want, "{name}");
        }
    }

    #[test]
    fn mean() {
        let cases = [
            ("single", sample(700.0, &[1441]), 1441.0),
            ("three", sample(700.0, &[1, 2, 6]), 3.0),
        ];
        for (name, s, want) in cases {
            assert_eq!(s.mean(), want, "{name}");
        }
    }

    #[test]
    fn csv_roundtrip() {
        let cases = [
            ("integral nm", sample(700.0, &[1441, 1432, 1433])),
            ("fractional nm", sample(702.5, &[1301, 1709, 1968])),
        ];
        for (name, s) in cases {
            assert_eq!(Sample::parse(&s.to_csv()), Some(s.clone()), "{name}");
        }
    }
}
