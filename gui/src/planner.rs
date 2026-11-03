use std::time::Duration;

use eframe::egui;
use spectraplot::plan::{GRATINGS, HV_VOLTS, INTEGRATION_MS, Problem, ScanPlan};
use spectraplot::presets::Presets;

/// What the user asked for in the planner panel.
pub enum PlannerAction {
    Start,
    Abort,
    SavePreset,
    DeletePreset,
}

/// Scans with at least this much pure integration time ask before starting.
const LONG_SCAN: Duration = Duration::from_secs(10 * 60);

/// The scan parameter panel, including the confirmation for long scans.
pub struct Planner {
    pub plan: ScanPlan,
    confirming: bool,
}

impl Planner {
    /// A planner showing `plan`.
    pub fn new(plan: ScanPlan) -> Self {
        Self {
            plan,
            confirming: false,
        }
    }

    /// Draws the panel; returns what the user asked for, if anything.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        connected: bool,
        scanning: bool,
        presets: &Presets,
    ) -> Option<PlannerAction> {
        choose_preset(ui, &mut self.plan, presets);
        let preset_action = name_row(ui, &mut self.plan.name, presets);
        params(ui, &mut self.plan);
        ui.separator();
        let direction = if self.plan.is_reversed() {
            ", downward"
        } else {
            ""
        };
        ui.label(format!(
            "{} steps{direction}, at least {}",
            self.plan.steps(),
            minutes_seconds(self.plan.integration_total())
        ));
        ui.monospace(self.plan.to_command());
        problems(ui, &self.plan.problems());
        ui.separator();
        if scanning {
            let abort = ui.button("abort scan").clicked();
            return abort.then_some(PlannerAction::Abort).or(preset_action);
        }
        let start = self.start_buttons(ui, connected && self.plan.is_runnable());
        start.then_some(PlannerAction::Start).or(preset_action)
    }

    fn start_buttons(&mut self, ui: &mut egui::Ui, enabled: bool) -> bool {
        if self.confirming {
            return self.confirm_long_scan(ui);
        }
        if !ui
            .add_enabled(enabled, egui::Button::new("start scan"))
            .clicked()
        {
            return false;
        }
        let long = self.plan.integration_total() >= LONG_SCAN;
        self.confirming = long;
        !long
    }

    fn confirm_long_scan(&mut self, ui: &mut egui::Ui) -> bool {
        let duration = minutes_seconds(self.plan.integration_total());
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!("this scan takes at least {duration}"),
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

fn choose_preset(ui: &mut egui::Ui, plan: &mut ScanPlan, presets: &Presets) {
    egui::ComboBox::from_id_salt("settings")
        .selected_text("presets…")
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if presets.items.is_empty() {
                ui.weak("none saved yet");
            }
            for preset in &presets.items {
                let current = !plan.name.is_empty() && plan.name == preset.name;
                if ui.selectable_label(current, &preset.name).clicked() {
                    *plan = preset.clone();
                }
            }
        });
}

fn name_row(ui: &mut egui::Ui, name: &mut String, presets: &Presets) -> Option<PlannerAction> {
    ui.horizontal(|ui| {
        ui.label("name");
        ui.text_edit_singleline(name);
    });
    let named = !name.trim().is_empty();
    let known = presets.contains(name);
    let mut action = None;
    ui.horizontal(|ui| {
        let save = if known {
            "update preset"
        } else {
            "save preset"
        };
        if ui.add_enabled(named, egui::Button::new(save)).clicked() {
            action = Some(PlannerAction::SavePreset);
        }
        if ui
            .add_enabled(known, egui::Button::new("delete preset"))
            .clicked()
        {
            action = Some(PlannerAction::DeletePreset);
        }
    });
    action
}

fn minutes_seconds(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{} min {:02} s", seconds / 60, seconds % 60)
}

fn params(ui: &mut egui::Ui, plan: &mut ScanPlan) {
    egui::Grid::new("plan").num_columns(2).show(ui, |ui| {
        ui.label("grating");
        grating_combo(ui, &mut plan.grating);
        ui.end_row();
        nm_row(ui, "start", &mut plan.start_nm);
        nm_row(ui, "end", &mut plan.end_nm);
        ui.label("");
        if ui.button("swap start/end").clicked() {
            *plan = plan.reversed();
        }
        ui.end_row();
        nm_row(ui, "step", &mut plan.step_nm);
        ui.label("HV");
        ui.add(
            egui::DragValue::new(&mut plan.hv)
                .range(HV_VOLTS)
                .suffix(" V"),
        );
        ui.end_row();
        ui.label("integration");
        ui.add(
            egui::DragValue::new(&mut plan.integration_ms)
                .range(INTEGRATION_MS)
                .suffix(" ms"),
        );
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
