/// Most recently used items, newest first, without duplicates.
#[derive(Debug, Clone, PartialEq)]
pub struct History<T> {
    pub items: Vec<T>,
    limit: usize,
}

impl<T: PartialEq> History<T> {
    /// An empty history keeping at most `limit` items.
    pub fn new(limit: usize) -> Self {
        Self {
            items: Vec::new(),
            limit,
        }
    }

    /// Reads one item per line; lines `parse_line` rejects are skipped.
    pub fn parse(text: &str, limit: usize, parse_line: impl Fn(&str) -> Option<T>) -> Self {
        let items = text
            .lines()
            .filter_map(|line| parse_line(line.trim()))
            .take(limit);
        Self {
            items: items.collect(),
            limit,
        }
    }

    /// Writes one item per line.
    pub fn to_text(&self, format: impl Fn(&T) -> String) -> String {
        self.items.iter().map(|item| format(item) + "\n").collect()
    }

    /// Moves `item` to the front, dropping duplicates and the oldest beyond the limit.
    pub fn push(&mut self, item: T) {
        self.items.retain(|existing| *existing != item);
        self.items.insert(0, item);
        self.items.truncate(self.limit);
    }

    /// The newest item.
    pub fn latest(&self) -> Option<&T> {
        self.items.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(limit: usize, items: &[u32]) -> History<u32> {
        History {
            items: items.to_vec(),
            limit,
        }
    }

    fn number(line: &str) -> Option<u32> {
        line.parse().ok()
    }

    #[test]
    fn parse() {
        let cases = [
            ("two", "1\n2\n", history(3, &[1, 2])),
            (
                "skips junk and blanks",
                "1\nx\n\n 2 \n",
                history(3, &[1, 2]),
            ),
            ("capped", "1\n2\n3\n4\n", history(3, &[1, 2, 3])),
            ("empty", "", history(3, &[])),
        ];
        for (name, text, want) in cases {
            assert_eq!(History::parse(text, 3, number), want, "{name}");
        }
    }

    #[test]
    fn push() {
        let cases = [
            ("into empty", history(3, &[]), 1, history(3, &[1])),
            ("new in front", history(3, &[1]), 2, history(3, &[2, 1])),
            (
                "existing moves up",
                history(3, &[1, 2]),
                2,
                history(3, &[2, 1]),
            ),
            (
                "oldest dropped",
                history(3, &[1, 2, 3]),
                4,
                history(3, &[4, 1, 2]),
            ),
        ];
        for (name, mut history, item, want) in cases {
            history.push(item);
            assert_eq!(history, want, "{name}");
        }
    }

    #[test]
    fn roundtrip() {
        let history = history(3, &[3, 1]);
        let text = history.to_text(u32::to_string);
        assert_eq!(text, "3\n1\n");
        assert_eq!(History::parse(&text, 3, number), history);
    }
}
