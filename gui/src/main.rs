mod args;
mod bubble;
mod clipboard;
mod console;
mod device;
mod document;
mod files;
mod help;
mod menu;
mod notes;
mod planner;

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;
use egui_plot::{HoverPosition, Line, LineStyle, Plot, PlotPoint, PlotPoints, Points, VLine};
use spectraplot::event::Event;
use spectraplot::history::History;
use spectraplot::plan::ScanPlan;
use spectraplot::presets::Presets;

use console::Console;
use device::Connection;
use document::Document;
use help::Help;
use menu::{Action, FileAction};
use notes::Notes;
use planner::{Planner, PlannerAction};

fn cursor_label(hover: &HoverPosition<'_>) -> Option<String> {
    let (HoverPosition::NearDataPoint { position, .. } | HoverPosition::Elsewhere { position }) =
        hover;
    Some(format!("{:.2} nm\n{:.0}", position.x, position.y))
}

/// How long after the current step's integration an abort may take to be confirmed.
const ABORT_MARGIN: Duration = Duration::from_secs(5);
const RETRY_EVERY: Duration = Duration::from_secs(2);

struct App {
    ctx: egui::Context,
    connection: Option<Connection>,
    console: Console,
    help: Help,
    planner: Planner,
    doc: Document,
    recent: History<PathBuf>,
    presets: Presets,
    scanning: bool,
    abort_deadline: Option<Instant>,
    retry: Option<Retry>,
    notes: Notes,
    editing: Option<usize>,
    focus_editor: bool,
    over_bubble: bool,
    grab: egui::Vec2,
    status: String,
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        let planner = Planner::new(files::load_last().unwrap_or_default());
        Self {
            ctx,
            connection: None,
            console: Console::default(),
            help: Help::default(),
            planner,
            doc: Document::default(),
            recent: files::load_recent(),
            presets: files::load_presets(),
            scanning: false,
            abort_deadline: None,
            retry: None,
            notes: Notes::default(),
            editing: None,
            focus_editor: false,
            over_bubble: false,
            grab: egui::Vec2::ZERO,
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
        let Some(connection) = &self.connection else {
            return;
        };
        let plan = &self.planner.plan;
        connection.send_line(&plan.to_command());
        let stem = format!("{}_{}", plan.file_stem(), timestamp());
        let notes = self.notes_for_next_scan();
        self.doc = Document::for_scan(stem);
        self.doc.scan.notes = notes;
        self.editing = None;
        self.scanning = true;
        self.abort_deadline = None;
        self.remember_last();
    }

    /// Notes written before the scan started belong to it; those of a finished scan do not.
    fn notes_for_next_scan(&mut self) -> String {
        if self.doc.scan.samples.is_empty() {
            std::mem::take(&mut self.doc.scan.notes)
        } else {
            String::new()
        }
    }

    fn send_command(&mut self, command: &str) {
        match &self.connection {
            Some(connection) => connection.send_line(command),
            None => self.console.push("not connected".into()),
        }
    }

    fn abort_scan(&mut self) {
        let Some(connection) = &self.connection else {
            return;
        };
        connection.send_line("abort scan");
        let wait = Duration::from_millis(self.planner.plan.integration_ms.into()) + ABORT_MARGIN;
        self.abort_deadline = Some(Instant::now() + wait);
        self.status = "abort sent, waiting for the device".into();
        wake_after(&self.ctx, wait);
    }

    fn give_up_on_abort(&mut self) {
        let overdue = self
            .abort_deadline
            .is_some_and(|deadline| Instant::now() >= deadline);
        if !self.scanning || !overdue {
            return;
        }
        self.scanning = false;
        self.abort_deadline = None;
        self.status = "device did not confirm the abort; stopped waiting".into();
        self.console.push(self.status.clone());
    }

    fn handle(&mut self, action: Action) {
        match action {
            Action::File(action) => self.handle_file(action),
            Action::Connect(path) => self.connect(&path),
            Action::Disconnect => self.disconnect(),
            Action::ToggleConsole => self.console.toggle(),
            Action::ToggleNotes => self.notes.open = !self.notes.open,
            Action::CopyData => self.copy_data(),
            Action::CopyCommand => {
                self.copy_to_clipboard(self.planner.plan.to_command(), "command")
            }
            Action::Help(title) => self.help.open_at(title),
            Action::DeviceHelp => self.device_help(),
        }
    }

    fn copy_to_clipboard(&mut self, text: String, what: &str) {
        self.ctx.copy_text(text);
        self.status = format!("copied {what}");
    }

    fn copy_data(&mut self) {
        let rows = self.doc.scan.samples.len();
        self.copy_to_clipboard(self.doc.scan.to_tsv(), &format!("{rows} rows"));
    }

    fn device_help(&mut self) {
        if !self.console.open {
            self.console.toggle();
        }
        self.send_command("help");
    }

    /// Builds the app inside eframe, connects and opens what the command line asks for.
    fn start(ctx: egui::Context, args: args::Args) -> Self {
        let mut app = Self::new(ctx);
        if let Some(port) = args.port.or_else(files::load_port) {
            app.connect_at_start(&port);
        }
        if let Some(Err(e)) = args.file.map(|f| app.open(Path::new(&f))) {
            app.status = e;
        }
        app
    }

    /// Keeps trying `path` every two seconds until it opens, e.g. once tio lets go of it.
    fn retry_connect(&mut self) {
        let Some(retry) = &self.retry else { return };
        if self.connection.is_some() {
            self.retry = None;
            return;
        }
        if Instant::now() < retry.next {
            wake_after(&self.ctx, retry.next - Instant::now());
            return;
        }
        let path = retry.path.clone();
        self.connect(&path);
        if self.connection.is_none() {
            self.retry = Some(Retry::after(&path));
            self.status =
                format!("{path} is busy or missing, retrying (close tio or pick another port)");
            wake_after(&self.ctx, RETRY_EVERY);
        }
    }

    fn connect(&mut self, path: &str) {
        self.retry = None;
        self.connection = None;
        let ctx = self.ctx.clone();
        match Connection::open(path, move || ctx.request_repaint()) {
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

    fn connect_at_start(&mut self, path: &str) {
        self.connect(path);
        if self.connection.is_none() {
            self.retry = Some(Retry::after(path));
            wake_after(&self.ctx, RETRY_EVERY);
        }
    }

    fn disconnect(&mut self) {
        self.retry = None;
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
        self.editing = None;
        self.remember(path);
        self.notes.open |= !self.doc.scan.notes.is_empty();
        if let Some(plan) = self
            .doc
            .scan
            .header
            .as_deref()
            .and_then(ScanPlan::from_header)
        {
            self.planner.plan = plan;
            self.remember_last();
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

    fn remember_last(&self) {
        if let Err(e) = files::store_last(&self.planner.plan.rounded()) {
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
            PlannerAction::SavePreset => self.save_preset(),
            PlannerAction::DeletePreset => self.delete_preset(),
        }
        if let Err(e) = files::store_presets(&self.presets) {
            self.status = format!("cannot store presets: {e}");
        }
    }

    fn save_preset(&mut self) {
        let plan = self.planner.plan.rounded();
        let name = plan.name.trim().to_string();
        let verb = if self.presets.contains(&name) {
            "updated"
        } else {
            "saved"
        };
        self.presets.save(plan);
        self.status = format!("{verb} preset {name}");
    }

    fn delete_preset(&mut self) {
        let name = self.planner.plan.name.trim().to_string();
        self.presets.remove(&name);
        self.status = format!("deleted preset {name}");
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

    fn notes_panel(&mut self, ui: &mut egui::Ui) {
        let mut open = self.notes.open;
        let changed = egui::Panel::bottom("notes")
            .resizable(true)
            .default_size(180.0)
            .drag_to_open(false)
            .show_collapsible(ui, &mut open, |ui| self.notes.show(ui, &mut self.doc.scan))
            .is_some_and(|response| response.inner);
        self.notes.open = open;
        self.doc.dirty |= changed;
    }

    fn planner_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.doc.title());
        ui.separator();
        let action = self
            .planner
            .show(ui, self.connection.is_some(), self.scanning, &self.presets);
        if let Some(action) = action {
            self.handle_planner(action);
        }
        ui.separator();
        let status = ui.small(&self.status);
        clipboard::copy_menu(&status, "copy message", &self.status);
    }

    /// The wavelength of the sample that lies closest to `position` on screen.
    fn nearest_on_screen(
        &self,
        shown: &egui_plot::PlotResponse<()>,
        position: egui::Pos2,
    ) -> Option<f64> {
        let distance = |s: &&spectraplot::sample::Sample| {
            let at = shown
                .transform
                .position_from_point(&PlotPoint::new(s.nm, s.mean()));
            at.distance_sq(position)
        };
        let samples = self.doc.scan.samples.iter();
        samples
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .map(|s| s.nm)
    }

    fn mark_near(&mut self, nearest: Option<f64>) {
        let Some(nearest) = nearest else {
            self.status = "no data to mark yet".into();
            return;
        };
        if self.doc.scan.mark_index(nearest).is_none() {
            self.doc.scan.set_mark(nearest, "");
            self.doc.dirty = true;
        }
        self.editing = self.doc.scan.mark_index(nearest);
        self.focus_editor = true;
    }

    fn marks_in_plot(&self, plot: &mut egui_plot::PlotUi<'_>) {
        let color = egui::Color32::from_rgb(230, 160, 40);
        for mark in &self.doc.scan.marks {
            plot.vline(
                VLine::new("mark", mark.nm)
                    .style(LineStyle::dashed_dense())
                    .color(color),
            );
            let Some(sample) = self.doc.scan.samples.iter().find(|s| s.nm == mark.nm) else {
                continue;
            };
            let at = [sample.nm, sample.mean()];
            plot.points(Points::new("mark", at).radius(4.0).color(color));
        }
    }

    /// Moves a mark to `target`, unless another mark sits there already.
    fn move_mark(&mut self, index: usize, target: Option<f64>) {
        let Some(target) = target else {
            return;
        };
        if self.doc.scan.mark_index(target).is_none() {
            self.doc.scan.marks[index].nm = target;
            self.doc.dirty = true;
        }
    }

    /// A ring on the point a double-click would mark.
    fn preview_target(&self, painter: &egui::Painter, shown: &egui_plot::PlotResponse<()>) {
        let Some(position) = shown.response.hover_pos() else {
            return;
        };
        let Some(nm) = self.nearest_on_screen(shown, position) else {
            return;
        };
        let Some(sample) = self.doc.scan.samples.iter().find(|s| s.nm == nm) else {
            return;
        };
        let at = shown
            .transform
            .position_from_point(&PlotPoint::new(nm, sample.mean()));
        let color = egui::Color32::from_rgb(230, 160, 40);
        painter.circle_stroke(at, 7.0, egui::Stroke::new(1.5, color));
    }

    /// Drawn after the plot, so the bubbles lie over the curve. Double-click a bubble to
    /// edit its text in place, right-click it to remove the mark.
    fn bubbles(&mut self, ui: &mut egui::Ui, shown: &egui_plot::PlotResponse<()>) {
        let painter = ui.painter().with_clip_rect(shown.response.rect);
        let mut finished = false;
        let mut removed = None;
        let mut started = None;
        let mut over = egui::Popup::is_any_open(ui.ctx());
        let mut moved = None;
        let mut dropped = false;
        for index in 0..self.doc.scan.marks.len() {
            let mark = &mut self.doc.scan.marks[index];
            let Some(sample) = self.doc.scan.samples.iter().find(|s| s.nm == mark.nm) else {
                continue;
            };
            let tip = shown
                .transform
                .position_from_point(&PlotPoint::new(sample.nm, sample.mean()));
            if self.editing == Some(index) {
                let body = bubble::draw(&painter, tip, &mark.text, true);
                let field = egui::TextEdit::singleline(&mut mark.text)
                    .frame(egui::Frame::NONE)
                    .text_color(bubble::ink())
                    .desired_width(body.width() - bubble::PADDING.x * 2.0);
                let response = ui.put(body.shrink2(bubble::PADDING), field);
                if std::mem::take(&mut self.focus_editor) {
                    response.request_focus();
                }
                self.doc.dirty |= response.changed();
                finished |= response.lost_focus();
                continue;
            }
            let text = match mark.text.is_empty() {
                true => format!("{:.1} nm", mark.nm),
                false => mark.text.clone(),
            };
            let body = bubble::draw(&painter, tip, &text, false);
            let id = egui::Id::new(("bubble", index));
            let response = ui.interact(body, id, egui::Sense::click_and_drag());
            let response = response.on_hover_cursor(egui::CursorIcon::Grab);
            over |= response.hovered();
            let pointer = response.interact_pointer_pos();
            if response.drag_started() {
                self.grab = pointer.map_or(egui::Vec2::ZERO, |p| p - tip);
            }
            if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                moved = pointer.map(|p| (index, p - self.grab));
            }
            dropped |= response.drag_stopped();
            if response.double_clicked() {
                started = Some(index);
            }
            response.context_menu(|ui| {
                if ui.button("remove mark").clicked() {
                    removed = Some(index);
                    ui.close();
                }
            });
        }
        self.over_bubble = over;
        if let Some((index, position)) = moved {
            let nearest = self.nearest_on_screen(shown, position);
            self.move_mark(index, nearest);
        }
        if dropped {
            self.doc.scan.sort_marks();
        }
        if !over {
            self.preview_target(&painter, shown);
        }
        if finished {
            self.editing = None;
        }
        if let Some(index) = removed {
            self.doc.scan.marks.remove(index);
            self.editing = None;
            self.doc.dirty = true;
        }
        if started.is_some() {
            self.editing = started;
            self.focus_editor = true;
        }
    }

    fn plot(&mut self, ui: &mut egui::Ui) {
        let plan = &self.planner.plan;
        let shown = Plot::new("scan")
            .include_x(plan.start_nm)
            .include_x(plan.end_nm)
            .allow_double_click_reset(false)
            .show_x(!self.over_bubble)
            .show_y(!self.over_bubble)
            .x_axis_label("nm")
            .y_axis_label("PMT")
            .label_formatter(cursor_label)
            .show(ui, |plot| {
                plot.points(Points::new("reads", self.read_points()).radius(1.5));
                plot.line(Line::new("mean", self.mean_points()));
                self.marks_in_plot(plot);
            });
        self.bubbles(ui, &shown);
        let response = &shown.response;
        if let Some(position) = response.hover_pos().filter(|_| response.double_clicked()) {
            self.mark_near(self.nearest_on_screen(&shown, position));
        }
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

/// A port that could not be opened at startup and is tried again until it works.
struct Retry {
    path: String,
    next: Instant,
}

impl Retry {
    fn after(path: &str) -> Self {
        Self {
            path: path.to_string(),
            next: Instant::now() + RETRY_EVERY,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.give_up_on_abort();
        self.retry_connect();
        clipboard::classic_keys(ui.ctx());
        let connected = self.connection.as_ref().map(|c| c.path.as_str());
        let action = egui::Panel::top("menu")
            .show(ui, |ui| {
                menu::bar(
                    ui,
                    &self.recent,
                    connected,
                    self.scanning,
                    self.console.open,
                    self.notes.open,
                    !self.doc.scan.samples.is_empty(),
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
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2)) {
            self.notes.open = !self.notes.open;
        }
        self.help.show(ui.ctx());
        self.console_panel(ui);
        self.notes_panel(ui);
        egui::Panel::left("planner").show(ui, |ui| self.planner_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.plot(ui));
    }

    fn on_exit(&mut self) {
        self.remember_last();
    }
}

/// Redraws the window after `wait`, even if nothing else happens until then.
fn wake_after(ctx: &egui::Context, wait: Duration) {
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(wait);
        ctx.request_repaint();
    });
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
    eframe::run_native(
        "spectraplot",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(App::start(cc.egui_ctx.clone(), args)))),
    )?;
    Ok(())
}
