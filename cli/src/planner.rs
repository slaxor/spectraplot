use eframe::egui;
use spectraplot::plan::{GRATINGS, HV_VOLTS, Problem, ScanPlan};

const MAX_READS: u8 = 20;

/// The scan parameter panel, including the grating-change confirmation.
#[derive(Default)]
pub struct Planner {
    pub plan: ScanPlan,
    confirming: bool,
}

impl Planner {
    /// Draws the panel; returns true when the user asked to start the scan.
    pub fn show(&mut self, ui: &mut egui::Ui, can_start: bool, grating_changes: bool) -> bool {
        params(ui, &mut self.plan);
        ui.separator();
        ui.label(format!("{} steps", self.plan.steps()));
        ui.monospace(self.plan.to_command());
        problems(ui, &self.plan.problems());
        ui.separator();
        self.start_buttons(ui, can_start && self.plan.is_runnable(), grating_changes)
    }

    fn start_buttons(&mut self, ui: &mut egui::Ui, enabled: bool, grating_changes: bool) -> bool {
        if self.confirming {
            return self.confirm_grating_change(ui);
        }
        if !ui
            .add_enabled(enabled, egui::Button::new("start scan"))
            .clicked()
        {
            return false;
        }
        self.confirming = grating_changes;
        !grating_changes
    }

    fn confirm_grating_change(&mut self, ui: &mut egui::Ui) -> bool {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "grating change can block the drive for minutes",
        );
        let mut start = false;
        ui.horizontal(|ui| {
            start = ui.button("start anyway").clicked();
            if start || ui.button("cancel").clicked() {
                self.confirming = false;
            }
        });
        start
    }
}

fn params(ui: &mut egui::Ui, plan: &mut ScanPlan) {
    egui::Grid::new("plan").num_columns(2).show(ui, |ui| {
        ui.label("grating");
        grating_combo(ui, &mut plan.grating);
        ui.end_row();
        nm_row(ui, "start", &mut plan.start_nm);
        nm_row(ui, "end", &mut plan.end_nm);
        nm_row(ui, "step", &mut plan.step_nm);
        ui.label("HV");
        ui.add(
            egui::DragValue::new(&mut plan.hv)
                .range(HV_VOLTS)
                .suffix(" V"),
        );
        ui.end_row();
        ui.label("reads");
        ui.add(egui::DragValue::new(&mut plan.reads).range(1..=MAX_READS));
        ui.end_row();
    });
}

fn grating_combo(ui: &mut egui::Ui, grating: &mut u8) {
    egui::ComboBox::from_id_salt("grating")
        .selected_text(format!("G{grating}"))
        .show_ui(ui, |ui| {
            for g in &GRATINGS {
                ui.selectable_value(grating, g.id, g.to_string());
            }
        });
}

fn nm_row(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.label(label);
    ui.add(
        egui::DragValue::new(value)
            .speed(0.1)
            .fixed_decimals(1)
            .suffix(" nm"),
    );
    ui.end_row();
}

fn problems(ui: &mut egui::Ui, problems: &[Problem]) {
    for problem in problems {
        let color = if problem.is_blocking() {
            ui.visuals().error_fg_color
        } else {
            ui.visuals().warn_fg_color
        };
        ui.colored_label(color, problem.to_string());
    }
}
