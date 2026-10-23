mod args;
mod console;
mod device;
mod document;
mod files;
mod help;
mod menu;
mod planner;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui;
use egui_plot::{HoverPosition, Line, Plot, PlotPoints, Points};
use spectraplot::event::Event;
use spectraplot::history::History;
use spectraplot::plan::ScanPlan;
use spectraplot::presets::Presets;

use console::Console;
use device::Connection;
use document::Document;
use help::Help;
use menu::{Action, FileAction};
use planner::{Planner, PlannerAction};

fn cursor_label(hover: &HoverPosition<'_>) -> Option<String> {
    let (HoverPosition::NearDataPoint { position, .. } | HoverPosition::Elsewhere { position }) =
        hover;
    Some(format!("{:.2} nm\n{:.0}", position.x, position.y))
}

struct App {
    connection: Option<Connection>,
    console: Console,
    help: Help,
    planner: Planner,
    doc: Document,
    recent: History<PathBuf>,
    plans: History<ScanPlan>,
    presets: Presets,
    scanning: bool,
    status: String,
}

impl App {
    fn new() -> Self {
        let plans = files::load_plans();
        let planner = Planner::new(plans.latest().cloned().unwrap_or_default());
        Self {
            connection: None,
            console: Console::default(),
            help: Help::default(),
            planner,
            doc: Document::default(),
            recent: files::load_recent(),
            plans,
            presets: files::load_presets(),
            scanning: false,
            status: "not connected, pick a port in the Port menu".into(),
        }
    }

    fn drain_events(&mut self) {
        let Some(connection) = &self.connection else {
            return;
        };
        let messages: Vec<_> = connection.messages.try_iter().collect();
        for message in messages {
            self.console.push(message.text);
            match message.event {
                Event::ScanDone => {
                    self.scanning = false;
                    self.status = "scan done".into();
                }
                Event::ScanStopped(line) => {
                    self.scanning = false;
                    self.status = line;
                }
                Event::Disconnected(reason) => {
                    self.connection = None;
                    self.scanning = false;
                    self.status = reason;
                }
                Event::Other(line) => self.status = line,
                event => self.doc.dirty |= self.doc.scan.apply(event),
            }
        }
    }

    fn start_scan(&mut self) {
        let Some(connection) = self.connection.as_mut() else {
            return;
        };
        let plan = &self.planner.plan;
        match connection.send_line(&plan.to_command()) {
            Ok(()) => {
                self.doc = Document::for_scan(format!("{}_{}", plan.file_stem(), timestamp()));
                self.scanning = true;
                self.remember_plan();
            }
            Err(e) => self.status = format!("send failed: {e}"),
        }
    }

    fn send_command(&mut self, command: &str) {
        let Some(connection) = self.connection.as_mut() else {
            self.console.push("not connected".into());
            return;
        };
        if let Err(e) = connection.send_line(command) {
            self.console.push(format!("send failed: {e}"));
        }
    }

    fn abort_scan(&mut self) {
        let Some(connection) = self.connection.as_mut() else {
            return;
        };
        if let Err(e) = connection.send_line("abort scan") {
            self.status = format!("send failed: {e}");
        }
    }

    fn handle(&mut self, action: Action) {
        match action {
            Action::File(action) => self.handle_file(action),
            Action::Connect(path) => self.connect(&path),
            Action::Disconnect => self.disconnect(),
            Action::ToggleConsole => self.console.toggle(),
            Action::Help(title) => self.help.open_at(title),
            Action::DeviceHelp => self.device_help(),
        }
    }

    fn device_help(&mut self) {
        if !self.console.open {
            self.console.toggle();
        }
        self.send_command("help");
    }

    fn connect(&mut self, path: &str) {
        self.connection = None;
        match Connection::open(path) {
            Ok(connection) => {
                self.connection = Some(connection);
                self.status = format!("connected to {path}");
                if let Err(e) = files::store_port(path) {
                    eprintln!("cannot store port: {e}");
                }
            }
            Err(e) => self.status = format!("cannot open {path}: {e}"),
        }
    }

    fn disconnect(&mut self) {
        self.connection = None;
        self.scanning = false;
        self.status = "disconnected".into();
    }

    fn handle_file(&mut self, action: FileAction) {
        if self.scanning && matches!(action, FileAction::Open | FileAction::OpenRecent(_)) {
            self.status = "cannot open while a scan is running".into();
            return;
        }
        let stem = self.doc.suggested_stem();
        let result = match action {
            FileAction::Open => files::pick_open().map(|p| self.open(&p)),
            FileAction::OpenRecent(p) => Some(self.open(&p)),
            FileAction::Save => self
                .doc
                .path
                .clone()
                .or_else(|| files::pick_save(&stem))
                .map(|p| self.save(&p)),
            FileAction::SaveAs => files::pick_save(&stem).map(|p| self.save(&p)),
            FileAction::Export => self
                .doc
                .export_path
                .clone()
                .or_else(|| files::pick_export(&stem))
                .map(|p| self.export(&p)),
            FileAction::ExportAs => files::pick_export(&stem).map(|p| self.export(&p)),
        };
        match result {
            None => self.status = "no file chosen".into(),
            Some(Err(e)) => self.status = e,
            Some(Ok(())) => {}
        }
    }

    fn open(&mut self, path: &Path) -> Result<(), String> {
        self.doc = Document::open(path).map_err(|e| failed("open", path, &e))?;
        self.remember(path);
        if let Some(plan) = self
            .doc
            .scan
            .header
            .as_deref()
            .and_then(ScanPlan::from_header)
        {
            self.planner.plan = plan;
            self.remember_plan();
        }
        self.status = format!("opened {}", path.display());
        Ok(())
    }

    fn save(&mut self, path: &Path) -> Result<(), String> {
        self.doc.save(path).map_err(|e| failed("save", path, &e))?;
        self.remember(path);
        self.status = format!("saved {}", path.display());
        Ok(())
    }

    fn export(&mut self, path: &Path) -> Result<(), String> {
        self.doc
            .export(path)
            .map_err(|e| failed("export", path, &e))?;
        self.status = format!("exported {}", path.display());
        Ok(())
    }

    fn remember(&mut self, path: &Path) {
        self.recent.push(absolute(path));
        if let Err(e) = files::store_recent(&self.recent) {
            eprintln!("cannot store recent files: {e}");
        }
    }

    fn remember_plan(&mut self) {
        self.plans.push(self.planner.plan.rounded());
        if let Err(e) = files::store_plans(&self.plans) {
            eprintln!("cannot store scan settings: {e}");
        }
    }

    fn handle_planner(&mut self, action: PlannerAction) {
        match action {
            PlannerAction::Start => {
                self.start_scan();
                return;
            }
            PlannerAction::Abort => {
                self.abort_scan();
                return;
            }
            PlannerAction::SavePreset => self.presets.save(self.planner.plan.rounded()),
            PlannerAction::DeletePreset => self.presets.remove(&self.planner.plan.name),
        }
        if let Err(e) = files::store_presets(&self.presets) {
            self.status = format!("cannot store presets: {e}");
        }
    }

    fn console_panel(&mut self, ui: &mut egui::Ui) {
        let mut open = self.console.open;
        let command = egui::Panel::top("console")
            .resizable(true)
            .default_size(300.0)
            .drag_to_open(false)
            .show_collapsible(ui, &mut open, |ui| self.console.show(ui))
            .and_then(|response| response.inner);
        self.console.open = open;
        if let Some(command) = command {
            self.send_command(&command);
        }
    }

    fn planner_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.doc.title());
        ui.separator();
        let action = self.planner.show(
            ui,
            self.connection.is_some(),
            self.scanning,
            &self.presets,
            &self.plans.items,
        );
        if let Some(action) = action {
            self.handle_planner(action);
        }
        ui.separator();
        ui.small(&self.status);
    }

    fn plot(&self, ui: &mut egui::Ui) {
        let plan = &self.planner.plan;
        Plot::new("scan")
            .include_x(plan.start_nm)
            .include_x(plan.end_nm)
            .x_axis_label("nm")
            .y_axis_label("PMT")
            .label_formatter(cursor_label)
            .show(ui, |plot| {
                plot.points(Points::new("reads", self.read_points()).radius(1.5));
                plot.line(Line::new("mean", self.mean_points()));
            });
    }

    fn mean_points(&self) -> PlotPoints<'static> {
        self.doc
            .scan
            .samples
            .iter()
            .map(|s| [s.nm, s.mean()])
            .collect()
    }

    fn read_points(&self) -> PlotPoints<'static> {
        self.doc
            .scan
            .samples
            .iter()
            .flat_map(|s| s.reads.iter().map(move |r| [s.nm, f64::from(*r)]))
            .collect()
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        let connected = self.connection.as_ref().map(|c| c.path.as_str());
        let action = egui::Panel::top("menu")
            .show(ui, |ui| {
                menu::bar(
                    ui,
                    &self.recent,
                    connected,
                    self.scanning,
                    self.console.open,
                )
            })
            .inner;
        if let Some(action) = action {
            self.handle(action);
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F12)) {
            self.console.toggle();
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.help.open_at("Getting started");
        }
        self.help.show(ui.ctx());
        self.console_panel(ui);
        egui::Panel::left("planner").show(ui, |ui| self.planner_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.plot(ui));
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }

    fn on_exit(&mut self) {
        self.remember_plan();
    }
}

fn timestamp() -> String {
    jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string()
}

fn failed(what: &str, path: &Path, e: &io::Error) -> String {
    format!("cannot {what} {}: {e}", path.display())
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = args::parse(std::env::args().skip(1))?;
    let mut app = App::new();
    if let Some(port) = args.port.or_else(files::load_port) {
        app.connect(&port);
    }
    if let Some(Err(e)) = args.file.map(|f| app.open(Path::new(&f))) {
        app.status = e;
    }
    eframe::run_native(
        "spectraplot",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )?;
    Ok(())
}
