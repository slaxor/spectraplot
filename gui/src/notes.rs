use eframe::egui;
use spectraplot::scan::Scan;

/// The panel under the plot: free notes on the measurement.
#[derive(Default)]
pub struct Notes {
    pub open: bool,
}

impl Notes {
    /// Draws the notes field; returns true when the user changed something.
    pub fn show(&mut self, ui: &mut egui::Ui, scan: &mut Scan) -> bool {
        egui::ScrollArea::vertical()
            .show(ui, |ui| {
                let edit = egui::TextEdit::multiline(&mut scan.notes)
                    .hint_text(
                        "notes: setup, goals, remarks. Marks: double-click a point in the plot",
                    )
                    .desired_rows(4)
                    .desired_width(f32::INFINITY);
                ui.add(edit).changed()
            })
            .inner
    }
}
