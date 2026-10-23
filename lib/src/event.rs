use crate::sample::Sample;

const SCAN_OUT: &str = "[scan:out]";

/// One line of spectractrl output, classified.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Header(String),
    Sample(Sample),
    ScanDone,
    ScanStopped(String),
    Disconnected(String),
    Other(String),
}

impl Event {
    /// Classifies a line of device output; blank lines yield `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        let cleaned = clean(raw);
        let line = cleaned.as_str();
        if line.is_empty() {
            return None;
        }
        if line.starts_with("[scan:done]") {
            return Some(Self::ScanDone);
        }
        if line.starts_with("[scan:fail]") || line.starts_with("[scan:abort]") {
            return Some(Self::ScanStopped(line.to_string()));
        }
        if let Some(header) = line.trim_start_matches(SCAN_OUT).trim().strip_prefix('#') {
            return Some(Self::Header(header.trim().to_string()));
        }
        Some(Sample::parse(line).map_or_else(|| Self::Other(line.to_string()), Self::Sample))
    }
}

/// The text a terminal would show for a received line: escape sequences removed,
/// an overwritten prompt dropped, surrounding whitespace trimmed.
pub fn clean(raw: &str) -> String {
    last_redraw(&strip_escapes(raw)).trim().to_string()
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
            (
                "scan failed",
                "> \r\x1b[K[scan:fail] PS-445 timed out\r\n",
                Some(Event::ScanStopped("[scan:fail] PS-445 timed out".into())),
            ),
            (
                "scan aborted",
                "[scan:abort] by request",
                Some(Event::ScanStopped("[scan:abort] by request".into())),
            ),
        ];
        for (name, line, want) in cases {
            assert_eq!(Event::parse(line), want, "{name}");
        }
    }

    #[test]
    fn clean() {
        let cases = [
            (
                "event",
                "> \r\x1b[K[scan:out] 700.0,1441\r\n",
                "[scan:out] 700.0,1441",
            ),
            ("echoed command", "> nop\r\n", "> nop"),
            ("plain reply", "ok\r\n", "ok"),
            ("blank", "\r\n", ""),
        ];
        for (name, raw, want) in cases {
            assert_eq!(super::clean(raw), want, "{name}");
        }
    }
}
