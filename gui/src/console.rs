use std::collections::VecDeque;

use eframe::egui;

const MAX_LINES: usize = 5000;
const MAX_HISTORY: usize = 50;

/// The drop-down console: everything the device sent, and a line to type commands.
#[derive(Default)]
pub struct Console {
    pub open: bool,
    focus_pending: bool,
    lines: VecDeque<String>,
    input: String,
    history: Vec<String>,
    browsing: Option<usize>,
}

impl Console {
    /// Opens or closes the console; an opened console takes the keyboard.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.focus_pending = self.open;
    }

    /// Appends a line to the log, dropping the oldest beyond the limit.
    pub fn push(&mut self, line: String) {
        if self.lines.len() == MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    /// Draws log and input; returns a command when the user pressed Enter.
    ///
    /// The input line is laid out first, from the bottom up, so the log gets
    /// exactly the height that is left and the panel never grows on its own.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<String> {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let command = self.input_line(ui);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| self.log(ui));
            command
        })
        .inner
    }

    fn log(&self, ui: &mut egui::Ui) {
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show_rows(ui, row_height, self.lines.len(), |ui, rows| {
                for line in self.lines.range(rows) {
                    ui.monospace(line);
                }
            });
    }

    fn input_line(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let edit = egui::TextEdit::singleline(&mut self.input)
            .font(egui::TextStyle::Monospace)
            .hint_text("command, Enter to send, F12 to close")
            .desired_width(f32::INFINITY);
        let response = ui.add(edit);
        if std::mem::take(&mut self.focus_pending) {
            response.request_focus();
        }
        if response.has_focus() {
            self.browse_history(ui);
        }
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !entered {
            return None;
        }
        response.request_focus();
        self.submit()
    }

    fn browse_history(&mut self, ui: &egui::Ui) {
        let (up, down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
            )
        });
        let next = match (up, down, self.browsing) {
            (true, _, None) => self.history.len().checked_sub(1),
            (true, _, Some(index)) => Some(index.saturating_sub(1)),
            (_, true, Some(index)) if index + 1 < self.history.len() => Some(index + 1),
            (_, true, Some(_)) => None,
            _ => return,
        };
        self.browsing = next;
        self.input = next
            .map(|index| self.history[index].clone())
            .unwrap_or_default();
    }

    fn submit(&mut self) -> Option<String> {
        let command = std::mem::take(&mut self.input).trim().to_string();
        self.browsing = None;
        if command.is_empty() {
            return None;
        }
        self.history.retain(|previous| *previous != command);
        self.history.push(command.clone());
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        Some(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_drops_oldest() {
        let mut console = Console::default();
        for i in 0..MAX_LINES + 2 {
            console.push(i.to_string());
        }
        assert_eq!(console.lines.len(), MAX_LINES);
        assert_eq!(console.lines.front().map(String::as_str), Some("2"));
    }

    #[test]
    fn submit() {
        let cases = [
            ("trimmed", vec![], "  nop ", Some("nop"), vec!["nop"]),
            ("empty", vec![], "   ", None, vec![]),
            (
                "repeat moves to end",
                vec!["status", "nop"],
                "status",
                Some("status"),
                vec!["nop", "status"],
            ),
        ];
        for (name, history, input, want, want_history) in cases {
            let mut console = Console {
                history: history.into_iter().map(String::from).collect(),
                input: input.into(),
                ..Console::default()
            };
            assert_eq!(console.submit().as_deref(), want, "{name}");
            assert_eq!(console.history, want_history, "{name}");
            assert!(console.input.is_empty(), "{name}");
        }
    }
}
