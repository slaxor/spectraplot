mod args;
mod device;
mod planner;

use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui;
use egui_plot::{HoverPosition, Line, Plot, PlotPoints, Points};
use serialport::SerialPort;
use specplot::event::Event;
use specplot::sample::Sample;

use planner::Planner;

type Port = Box<dyn SerialPort>;

fn cursor_label(hover: &HoverPosition<'_>) -> Option<String> {
    let (HoverPosition::NearDataPoint { position, .. } | HoverPosition::Elsewhere { position }) =
        hover;
    Some(format!("{:.2} nm\n{:.0}", position.x, position.y))
}

struct App {
    events: Receiver<Event>,
    port: Option<Port>,
    planner: Planner,
    samples: Vec<Sample>,
    scanning: bool,
    last_grating: Option<u8>,
    status: String,
}

impl App {
    fn new(events: Receiver<Event>, port: Option<Port>) -> Self {
        let status = match port {
            Some(_) => "connected".into(),
            None => "replay mode, start with --port to scan".into(),
        };
        Self {
            events,
            port,
            planner: Planner::default(),
            samples: Vec::new(),
            scanning: false,
            last_grating: None,
            status,
        }
    }

    fn drain_events(&mut self) {
        for event in self.events.try_iter() {
            match event {
                Event::Sample(sample) => self.samples.push(sample),
                Event::ScanDone => self.scanning = false,
                Event::Other(line) => self.status = line,
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
                self.samples.clear();
                self.scanning = true;
                self.last_grating = Some(plan.grating);
            }
            Err(e) => self.status = format!("send failed: {e}"),
        }
    }

    fn planner_panel(&mut self, ui: &mut egui::Ui) {
        let can_start = self.port.is_some() && !self.scanning;
        let grating_changes = self.last_grating != Some(self.planner.plan.grating);
        if self.planner.show(ui, can_start, grating_changes) {
            self.start_scan();
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
        self.samples.iter().map(|s| [s.nm, s.mean()]).collect()
    }

    fn read_points(&self) -> PlotPoints<'static> {
        self.samples
            .iter()
            .flat_map(|s| s.reads.iter().map(move |r| [s.nm, f64::from(*r)]))
            .collect()
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        egui::Panel::left("planner").show(ui, |ui| self.planner_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.plot(ui));
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }
}

fn connect(
    port: Option<String>,
    csv: Option<File>,
) -> serialport::Result<(Receiver<Event>, Option<Port>)> {
    let Some(path) = port else {
        return Ok((
            device::spawn_reader(BufReader::new(std::io::stdin()), csv),
            None,
        ));
    };
    let port = device::open_port(&path)?;
    let reader = BufReader::new(port.try_clone()?);
    Ok((device::spawn_reader(reader, csv), Some(port)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args::parse(std::env::args().skip(1))?;
    let csv = args.csv.map(File::create).transpose()?;
    let (events, port) = connect(args.port, csv)?;
    let app = App::new(events, port);
    eframe::run_native(
        "specplot",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )?;
    Ok(())
}
