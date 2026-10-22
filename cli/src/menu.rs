use std::path::PathBuf;

use eframe::egui;
use spectraplot::recent::Recent;

/// What the user picked from the File menu.
pub enum Action {
    Open,
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    Export,
    ExportAs,
}

/// Draws the menu bar; returns the chosen action, if any.
pub fn file_menu(ui: &mut egui::Ui, recent: &Recent) -> Option<Action> {
    let mut action = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            action = entries(ui, recent);
        });
    });
    action
}

fn entries(ui: &mut egui::Ui, recent: &Recent) -> Option<Action> {
    let mut action = None;
    if ui.button("Open…").clicked() {
        action = Some(Action::Open);
    }
    ui.add_enabled_ui(!recent.paths.is_empty(), |ui| {
        ui.menu_button("Open Recent", |ui| {
            for path in &recent.paths {
                if ui.button(path.display().to_string()).clicked() {
                    action = Some(Action::OpenRecent(path.clone()));
                }
            }
        });
    });
    ui.separator();
    if ui.button("Save").clicked() {
        action = Some(Action::Save);
    }
    if ui.button("Save As…").clicked() {
        action = Some(Action::SaveAs);
    }
    ui.separator();
    if ui.button("Export").clicked() {
        action = Some(Action::Export);
    }
    if ui.button("Export As…").clicked() {
        action = Some(Action::ExportAs);
    }
    action
}
