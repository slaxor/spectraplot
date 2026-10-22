mod args;
mod device;
mod document;
mod files;
mod menu;
mod planner;

use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui;
use egui_plot::{HoverPosition, Line, Plot, PlotPoints, Points};
use serialport::SerialPort;
use spectraplot::event::Event;
use spectraplot::history::History;
use spectraplot::plan::ScanPlan;
use spectraplot::presets::Presets;

use document::Document;
use menu::Action;
use planner::{Planner, PlannerAction};

type Port = Box<dyn SerialPort>;

fn cursor_label(hover: &HoverPosition<'_>) -> Option<String> {
    let (HoverPosition::NearDataPoint { position, .. } | HoverPosition::Elsewhere { position }) =
        hover;
    Some(format!("{:.2} nm\n{:.0}", position.x, position.y))
}

struct App {
    events: Option<Receiver<Event>>,
    port: Option<Port>,
    planner: Planner,
    doc: Document,
    recent: History<PathBuf>,
    plans: History<ScanPlan>,
    presets: Presets,
    scanning: bool,
    last_grating: Option<u8>,
    status: String,
}

impl App {
    fn new(port: Option<Port>, events: Option<Receiver<Event>>) -> Self {
        let status = match port {
            Some(_) => "connected".into(),
            None => "no device, start with --port to scan".into(),
        };
        let plans = files::load_plans();
        let planner = Planner::new(plans.latest().cloned().unwrap_or_default());
        Self {
            events,
            port,
            planner,
            doc: Document::default(),
            recent: files::load_recent(),
            plans,
            presets: files::load_presets(),
            scanning: false,
            last_grating: None,
            status,
        }
    }

    fn drain_events(&mut self) {
        let Some(events) = &self.events else { return };
        for event in events.try_iter() {
            match event {
                Event::ScanDone => {
                    self.scanning = false;
                    self.status = "scan done".into();
                }
                Event::Other(line) => self.status = line,
                event => self.doc.dirty |= self.doc.scan.apply(event),
            }
        }
    }

    fn start_scan(&mut self) {
        let Some(port) = self.port.as_mut() else {
            return;
        };
        let plan = &self.planner.plan;
        match device::send_line(port.as_mut(), &plan.to_command()) {
            Ok(()) => {
                self.doc = Document::for_scan(format!("{}_{}", plan.file_stem(), timestamp()));
                self.scanning = true;
                self.last_grating = Some(plan.grating);
                self.remember_plan();
            }
            Err(e) => self.status = format!("send failed: {e}"),
        }
    }

    fn handle(&mut self, action: Action) {
        if self.scanning && matches!(action, Action::Open | Action::OpenRecent(_)) {
            self.status = "cannot open while a scan is running".into();
            return;
        }
        let stem = self.doc.suggested_stem();
        let result = match action {
            Action::Open => files::pick_open().map(|p| self.open(&p)),
            Action::OpenRecent(p) => Some(self.open(&p)),
            Action::Save => self
                .doc
                .path
                .clone()
                .or_else(|| files::pick_save(&stem))
                .map(|p| self.save(&p)),
            Action::SaveAs => files::pick_save(&stem).map(|p| self.save(&p)),
            Action::Export => self
                .doc
                .export_path
                .clone()
                .or_else(|| files::pick_export(&stem))
                .map(|p| self.export(&p)),
            Action::ExportAs => files::pick_export(&stem).map(|p| self.export(&p)),
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
            PlannerAction::SavePreset => self.presets.save(self.planner.plan.rounded()),
            PlannerAction::DeletePreset => self.presets.remove(&self.planner.plan.name),
        }
        if let Err(e) = files::store_presets(&self.presets) {
            self.status = format!("cannot store presets: {e}");
        }
    }

    fn planner_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.doc.title());
        ui.separator();
        let can_start = self.port.is_some() && !self.scanning;
        let grating_changes = self.last_grating != Some(self.planner.plan.grating);
        let action = self.planner.show(
            ui,
            can_start,
            grating_changes,
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
        Plot::new("scan")
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
        let action = egui::Panel::top("menu")
            .show(ui, |ui| menu::file_menu(ui, &self.recent))
            .inner;
        if let Some(action) = action {
            self.handle(action);
        }
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

fn connect(port: Option<String>) -> serialport::Result<(Option<Port>, Option<Receiver<Event>>)> {
    let Some(path) = port else {
        return Ok((None, None));
    };
    let port = device::open_port(&path)?;
    let reader = BufReader::new(port.try_clone()?);
    Ok((Some(port), Some(device::spawn_reader(reader))))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = args::parse(std::env::args().skip(1))?;
    let (port, events) = connect(args.port)?;
    let mut app = App::new(port, events);
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
