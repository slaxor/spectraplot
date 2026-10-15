use crate::sample::Sample;

/// One line of spectractrl output, classified.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Sample(Sample),
    ScanDone,
    Other(String),
}

impl Event {
    /// Classifies a line of device output; blank lines yield `None`.
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        if line.starts_with("[scan:done]") {
            return Some(Self::ScanDone);
        }
        Some(Sample::parse(line).map_or_else(|| Self::Other(line.to_string()), Self::Sample))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let sample = Sample {
            nm: 700.1,
            reads: vec![1433, 1346, 1388],
        };
        let cases = [
            (
                "sample",
                "[scan:out] 700.1,1433,1346,1388",
                Some(Event::Sample(sample)),
            ),
            ("done", "[scan:done] 501 steps", Some(Event::ScanDone)),
            (
                "header",
                "[scan:out] # grating 2",
                Some(Event::Other("[scan:out] # grating 2".into())),
            ),
            (
                "status",
                "  scan started\r\n",
                Some(Event::Other("scan started".into())),
            ),
            ("blank", " \r\n", None),
        ];
        for (name, line, want) in cases {
            assert_eq!(Event::parse(line), want, "{name}");
        }
    }
}
