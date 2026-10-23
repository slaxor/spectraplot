use eframe::egui;

/// One piece of a help section.
pub enum Block {
    Text(&'static str),
    Code(&'static str),
    Table(&'static [(&'static str, &'static str)]),
}

/// A titled page of the help window.
pub struct Section {
    pub title: &'static str,
    pub body: &'static [Block],
}

pub const SECTIONS: &[Section] = &[
    Section {
        title: "Getting started",
        body: &[
            Block::Text(
                "spectraplot drives a SpectraPro-275 monochromator and a PS-445 PMT \
                 through the spectractrl firmware and plots the result live.",
            ),
            Block::Table(&[
                ("1. Port", "pick the spectractrl USB port in the Port menu"),
                (
                    "2. Plan",
                    "choose grating, range, step, HV and integration time",
                ),
                ("3. Start", "press start scan and watch the plot fill in"),
                ("4. Save", "File > Save As writes a .dat file"),
            ]),
            Block::Text(
                "Nothing on the plot? Check that the detector shutter is open: a closed \
                 shutter gives a flat, noisy baseline with no lines at all.",
            ),
        ],
    },
    Section {
        title: "Planner",
        body: &[
            Block::Table(&[
                (
                    "settings…",
                    "presets by name, then the last 20 settings used",
                ),
                (
                    "name",
                    "optional; names a preset and the suggested file name",
                ),
                ("grating G1", "600 l/mm, blaze 300 nm, best 200–450 nm"),
                ("grating G2", "600 l/mm, blaze 750 nm, best 500–1100 nm"),
                ("grating G3", "1200 l/mm, blaze 500 nm, best 330–750 nm"),
                (
                    "start / end / step",
                    "nm with exactly one decimal; the drive misreads anything else",
                ),
                ("HV", "PMT voltage, 0–1000 V"),
                ("integration", "PMT integration time per step in ms"),
            ]),
            Block::Text(
                "Red lines block the scan, yellow ones are warnings (outside the grating's \
                 optimum or the PMT's 185–900 nm). The line below the fields is the exact \
                 command that will be sent.",
            ),
            Block::Text(
                "Scans with at least 10 minutes of pure integration time ask once before \
                 they start. While a scan runs, abort scan stops it.",
            ),
            Block::Text(
                "The planner always opens with the settings used last; opening a file \
                 loads the settings from its header.",
            ),
        ],
    },
    Section {
        title: "Plot",
        body: &[
            Block::Table(&[
                ("dots", "every single reading"),
                ("line", "mean of the readings per step"),
                ("drag", "pan"),
                ("Ctrl + scroll", "zoom"),
                ("right drag", "zoom into a box"),
                ("double click", "back to the planner's range"),
            ]),
            Block::Text(
                "The x axis always spans the planner's start and end, so a running scan \
                 fills a fixed frame. Hovering shows wavelength and reading, snapped to \
                 the nearest point.",
            ),
        ],
    },
    Section {
        title: "Console",
        body: &[
            Block::Text(
                "F12 drops down a console with everything the device sends. Type \
                 spectractrl commands and press Enter; Help > Device commands asks the \
                 firmware for its own list.",
            ),
            Block::Table(&[
                ("Enter", "send the line"),
                ("Up / Down", "walk through the last 50 commands"),
                ("abort scan", "stop the running scan"),
                ("status", "local state of spectractrl and the devices"),
            ]),
            Block::Text(
                "Everything typed goes to the same port as the scans, so be careful \
                 while a scan is running.",
            ),
        ],
    },
    Section {
        title: "Files",
        body: &[
            Block::Table(&[
                (
                    ".dat",
                    "native format: a # header line, then nm,reading,… per step",
                ),
                (
                    ".csv",
                    "export: nm, every reading and the mean, with column names",
                ),
                (".svg", "export: the mean as a plot, for documents"),
            ]),
            Block::Text(
                "Open also reads raw tio logs. Save and Export reuse the last file name; \
                 the As… entries always ask. New scans suggest <name or parameters>_<time>.",
            ),
            Block::Text("Settings live in ~/.local/state/spectraplot:"),
            Block::Table(&[
                ("recent", "recently opened files"),
                ("plans", "recently used scan settings"),
                ("presets", "named scan settings"),
                ("port", "the serial port to reconnect to"),
            ]),
        ],
    },
    Section {
        title: "Keys",
        body: &[Block::Table(&[
            ("F1", "this help"),
            ("F12", "console"),
            ("Enter", "send the console line"),
            ("Up / Down", "console history"),
        ])],
    },
    Section {
        title: "Measuring tips",
        body: &[
            Block::Text(
                "Set entrance and exit slit to the same width. The wider slit sets the \
                 resolution, the narrower one the signal, so unequal slits only cost.",
            ),
            Block::Text(
                "Above about twice the shortest wavelength the source emits, second order \
                 light shows up: at 1080 nm you see the 540 nm line again. A long-pass \
                 filter (GG395, OG550) in front of the entrance slit removes it.",
            ),
            Block::Text(
                "Each grating has its own wavelength offset. Neon lines such as 540.06, \
                 703.24, 743.89 and 748.89 nm make good references.",
            ),
        ],
    },
    Section {
        title: "Command line",
        body: &[
            Block::Code("spectraplot [--port DEVICE] [FILE]"),
            Block::Text(
                "--port connects right away instead of using the remembered port; FILE \
                 is opened at start. RUST_LOG=rfd=debug and similar turn on logging.",
            ),
        ],
    },
    Section {
        title: "About",
        body: &[
            Block::Text(concat!("spectraplot ", env!("CARGO_PKG_VERSION"))),
            Block::Text("Desktop front end for the spectractrl firmware."),
            Block::Code("https://github.com/slaxor/spectractrl"),
        ],
    },
];

/// The help window and the page it shows.
#[derive(Default)]
pub struct Help {
    pub open: bool,
    page: usize,
}

impl Help {
    /// Opens the window at the page called `title`, or the first page.
    pub fn open_at(&mut self, title: &str) {
        self.open = true;
        self.page = SECTIONS.iter().position(|s| s.title == title).unwrap_or(0);
    }

    /// Draws the window while it is open.
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        egui::Window::new("spectraplot help")
            .open(&mut open)
            .default_size([680.0, 420.0])
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    self.contents(ui);
                    ui.separator();
                    page(ui, &SECTIONS[self.page]);
                });
            });
        self.open = open;
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.set_width(130.0);
            for (index, section) in SECTIONS.iter().enumerate() {
                if ui
                    .selectable_label(self.page == index, section.title)
                    .clicked()
                {
                    self.page = index;
                }
            }
        });
    }
}

fn page(ui: &mut egui::Ui, section: &Section) {
    ui.vertical(|ui| page_contents(ui, section));
}

fn page_contents(ui: &mut egui::Ui, section: &Section) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(section.title);
        ui.add_space(4.0);
        for (index, block) in section.body.iter().enumerate() {
            render(ui, block, index);
            ui.add_space(6.0);
        }
    });
}

fn render(ui: &mut egui::Ui, block: &Block, index: usize) {
    match block {
        Block::Text(text) => {
            ui.label(*text);
        }
        Block::Code(code) => {
            ui.code(*code);
        }
        Block::Table(rows) => {
            egui::Grid::new(("help table", index))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (key, description) in *rows {
                        ui.strong(*key);
                        ui.label(*description);
                        ui.end_row();
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_have_unique_titles_and_content() {
        for (index, section) in SECTIONS.iter().enumerate() {
            assert!(!section.body.is_empty(), "{} is empty", section.title);
            let duplicate = SECTIONS[..index].iter().any(|s| s.title == section.title);
            assert!(!duplicate, "{} appears twice", section.title);
        }
    }

    #[test]
    fn open_at() {
        let cases = [
            ("Console", 3),
            ("About", SECTIONS.len() - 1),
            ("missing", 0),
        ];
        for (title, want) in cases {
            let mut help = Help::default();
            help.open_at(title);
            assert!(help.open, "{title}");
            assert_eq!(help.page, want, "{title}");
        }
    }
}
