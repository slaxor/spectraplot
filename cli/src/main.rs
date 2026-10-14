use std::fs::File;
use std::io::{BufRead, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui;
use egui_plot::{HoverPosition, Line, Plot, PlotPoints, Points};
use specplot::sample::Sample;

fn append_csv(csv: &mut File, sample: &Sample) {
    if let Err(e) = writeln!(csv, "{}", sample.to_csv()) {
        eprintln!("csv write failed: {e}");
    }
}

fn cursor_label(hover: &HoverPosition<'_>) -> Option<String> {
    let (HoverPosition::NearDataPoint { position, .. } | HoverPosition::Elsewhere { position }) =
        hover;
    Some(format!("{:.2} nm\n{:.0}", position.x, position.y))
}

fn spawn_stdin_reader(mut csv: Option<File>) -> Receiver<Sample> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let Some(sample) = Sample::parse(&line) else { continue };
            if let Some(csv) = csv.as_mut() {
                append_csv(csv, &sample);
            }
            if tx.send(sample).is_err() {
                break;
            }
        }
    });
    rx
}

struct App {
    rx: Receiver<Sample>,
    samples: Vec<Sample>,
}

impl App {
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
        self.samples.extend(self.rx.try_iter());
        egui::CentralPanel::default().show(ui, |ui| {
            Plot::new("scan")
                .x_axis_label("nm")
                .y_axis_label("PMT")
                .label_formatter(cursor_label)
                .show(ui, |plot| {
                    plot.points(Points::new("reads", self.read_points()).radius(1.5));
                    plot.line(Line::new("mean", self.mean_points()));
                });
        });
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csv = std::env::args().nth(1).map(File::create).transpose()?;
    let app = App { rx: spawn_stdin_reader(csv), samples: Vec::new() };
    eframe::run_native(
        "specplot",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )?;
    Ok(())
}
