use crate::sample::Sample;

const SCAN_OUT: &str = "[scan:out]";

/// One line of spectractrl output, classified.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Header(String),
    Sample(Sample),
    ScanDone,
    Other(String),
}

impl Event {
    /// Classifies a line of device output; blank lines yield `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        let cleaned = strip_escapes(raw);
        let line = last_redraw(&cleaned).trim();
        if line.is_empty() {
            return None;
        }
        if line.starts_with("[scan:done]") {
            return Some(Self::ScanDone);
        }
        if let Some(header) = line.trim_start_matches(SCAN_OUT).trim().strip_prefix('#') {
            return Some(Self::Header(header.trim().to_string()));
        }
        Some(Sample::parse(line).map_or_else(|| Self::Other(line.to_string()), Self::Sample))
    }
}

/// Removes ANSI CSI sequences like the `ESC [ K` the console sends before each event.
fn strip_escapes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        if chars.next() == Some('[') {
            chars.by_ref().find(|c| ('@'..='~').contains(c));
        }
    }
    out
}

/// Keeps what follows the last carriage return, dropping a prompt the line overwrote.
fn last_redraw(line: &str) -> &str {
    let line = line.trim_end();
    line.rsplit('\r').next().unwrap_or(line)
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
                "[scan:out] # grating 2, 700.0 nm",
                Some(Event::Header("grating 2, 700.0 nm".into())),
            ),
            (
                "bare header",
                "# grating 3",
                Some(Event::Header("grating 3".into())),
            ),
            (
                "status",
                "  scan started\r\n",
                Some(Event::Other("scan started".into())),
            ),
            ("blank", " \r\n", None),
            (
                "console framing",
                "> \r\x1b[K[scan:out] 700.0,1441\r\n",
                Some(Event::Sample(Sample {
                    nm: 700.0,
                    reads: vec![1441],
                })),
            ),
            (
                "framed header",
                "\r\x1b[K[scan:out] # grating 2\r\n",
                Some(Event::Header("grating 2".into())),
            ),
            (
                "framed done",
                "> \r\x1b[K[scan:done] 501 steps\r\n",
                Some(Event::ScanDone),
            ),
            ("bare prompt", "> ", Some(Event::Other(">".into()))),
        ];
        for (name, line, want) in cases {
            assert_eq!(Event::parse(line), want, "{name}");
        }
    }
}
