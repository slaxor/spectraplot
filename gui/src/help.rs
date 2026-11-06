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
                ("presets…", "the presets you saved; a click loads one"),
                (
                    "name",
                    "optional; names the suggested file name and, with save preset, a preset",
                ),
                (
                    "save / update preset",
                    "stores the settings under the name; a name exists once",
                ),
                (
                    "delete preset",
                    "forgets the preset with the name in the field",
                ),
                (
                    "swap start/end",
                    "reverses the scan; a start above the end scans downward",
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
                "Nothing becomes a preset unless you press save preset. Only the settings \
                 used last are remembered automatically: the planner opens with them, and \
                 opening a file loads the settings from its header.",
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
                    ".spectra",
                    "native format: a # header line, #> notes, #@ marks, then nm,reading,… per step",
                ),
                (
                    ".dat",
                    "the same text under the old name; still opens and saves",
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
            Block::Text(
                "Notes (F2) describe setup and goals and can be as long as you like. A mark \
                 comments one measurement point: double-click near the point in the plot and \
                 type into the bubble that appears. A ring shows which point a \
                 double-click would mark. Double-click a bubble to edit it later, drag it to \
                 move the mark to another point, right-click it to remove the mark. Both are saved in the file as comment lines, so gnuplot and other \
                 tools skip them.",
            ),
            Block::Text("Settings live in ~/.local/state/spectraplot:"),
            Block::Table(&[
                ("recent", "recently opened files"),
                ("last", "the scan settings used last"),
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
            ("F2", "notes and marks"),
            (
                "double-click in the plot",
                "mark the nearest point and type its comment",
            ),
            ("double-click a bubble", "edit its text in place"),
            ("drag a bubble", "move the mark to another point"),
            ("right-click a bubble", "remove the mark"),
            ("Enter", "send the console line"),
            ("Up / Down", "console history"),
            ("Ctrl+C / V / X", "copy, paste, cut in text fields"),
            (
                "Ctrl+Insert, Shift+Insert, Shift+Delete",
                "copy, paste, cut (classic keys)",
            ),
            (
                "Right click",
                "copy the console log, status line, command line",
            ),
            (
                "Edit menu",
                "copy the scan as TSV (pastes into a spreadsheet), copy the command",
            ),
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

/// The page as plain text, for the clipboard.
fn section_text(section: &Section) -> String {
    let mut text = format!("{}\n\n", section.title);
    for block in section.body {
        match block {
            Block::Text(t) | Block::Code(t) => text.push_str(t),
            Block::Table(rows) => {
                for (key, description) in *rows {
                    text.push_str(&format!("{key}\t{description}\n"));
                }
            }
        }
        text.push_str("\n\n");
    }
    text.trim_end().to_string() + "\n"
}

fn page_contents(ui: &mut egui::Ui, section: &Section) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading(section.title);
            if ui.small_button("copy page").clicked() {
                ui.ctx().copy_text(section_text(section));
            }
        });
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
    fn section_text_has_title_and_every_block() {
        for section in SECTIONS {
            let text = section_text(section);
            assert!(text.starts_with(section.title), "{}", section.title);
            for block in section.body {
                if let Block::Text(t) | Block::Code(t) = block {
                    assert!(text.contains(t), "{}", section.title);
                }
            }
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
