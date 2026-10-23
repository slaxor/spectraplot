use std::path::PathBuf;

use eframe::egui;
use spectraplot::history::History;

use crate::device;

/// What the user picked from the menu bar.
pub enum Action {
    File(FileAction),
    Connect(String),
    Disconnect,
    ToggleConsole,
}

/// An entry of the File menu.
pub enum FileAction {
    Open,
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    Export,
    ExportAs,
}

/// Draws the menu bar; returns the chosen action, if any.
/// `connected` is the open port; ports cannot be switched while a scan runs.
pub fn bar(
    ui: &mut egui::Ui,
    recent: &History<PathBuf>,
    connected: Option<&str>,
    scanning: bool,
    console_open: bool,
) -> Option<Action> {
    let mut action = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            action = file_entries(ui, recent).map(Action::File)
        });
        ui.menu_button("Port", |ui| action = port_entries(ui, connected, scanning));
        ui.menu_button("View", |ui| {
            let console = egui::Button::selectable(console_open, "Console  F12");
            if ui.add(console).clicked() {
                action = Some(Action::ToggleConsole);
            }
        });
    });
    action
}

fn port_entries(ui: &mut egui::Ui, connected: Option<&str>, scanning: bool) -> Option<Action> {
    let mut action = None;
    let ports = device::list_ports();
    if ports.is_empty() {
        ui.weak("no USB serial ports found");
    }
    if let Some(path) = connected.filter(|path| !ports.iter().any(|p| p.path == *path)) {
        ui.add_enabled(false, egui::Button::selectable(true, path));
    }
    for port in ports {
        let is_open = connected == Some(port.path.as_str());
        let entry = egui::Button::selectable(is_open, port.label);
        if ui.add_enabled(!scanning, entry).clicked() {
            action = Some(Action::Connect(port.path));
        }
    }
    ui.separator();
    if ui
        .add_enabled(connected.is_some(), egui::Button::new("Disconnect"))
        .clicked()
    {
        action = Some(Action::Disconnect);
    }
    action
}

fn file_entries(ui: &mut egui::Ui, recent: &History<PathBuf>) -> Option<FileAction> {
    let mut action = None;
    if ui.button("Open…").clicked() {
        action = Some(FileAction::Open);
    }
    ui.add_enabled_ui(!recent.items.is_empty(), |ui| {
        ui.menu_button("Open Recent", |ui| {
            for path in &recent.items {
                if ui.button(path.display().to_string()).clicked() {
                    action = Some(FileAction::OpenRecent(path.clone()));
                }
            }
        });
    });
    ui.separator();
    if ui.button("Save").clicked() {
        action = Some(FileAction::Save);
    }
    if ui.button("Save As…").clicked() {
        action = Some(FileAction::SaveAs);
    }
    ui.separator();
    if ui.button("Export").clicked() {
        action = Some(FileAction::Export);
    }
    if ui.button("Export As…").clicked() {
        action = Some(FileAction::ExportAs);
    }
    action
}
