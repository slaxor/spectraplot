# spectraplot

Desktop front end for an Acton Research **SpectraPro-275** monochromator with
**SpectraDrive** and a **PS-445** photomultiplier interface. It talks to the
[spectractrl](https://github.com/slaxor/spectractrl) firmware over USB, plans
and runs wavelength scans, and plots them live.

```
SpectraPro-275 ── SpectraDrive ─┐
                                ├─ RS-232 ── spectractrl (RP2040) ── USB ── spectraplot
R928 PMT ──────── PS-445 ───────┘
```

## Features

- **Scan planner** with grating, range, step, PMT voltage and integration time,
  validated before anything is sent: one decimal for wavelengths, drive and
  grating limits, PMT range
- **Presets and history**: name settings you use often, the last 20 settings are
  always one click away, and the planner opens with the last one
- **Live plot** with every reading and the mean per step, an x axis fixed to the
  planned range, and a cursor read-out of wavelength and value
- **Quake-style console** (F12) showing everything the device sends, with a
  command line and history
- **Files**: save scans as `.dat`, export `.csv` tables or `.svg` plots, open
  saved scans and raw `tio` logs, recent files
- **Port menu** listing the USB serial ports present right now, remembering the
  last one
- **Help** (F1) with a guide, key list and measuring tips

## Getting started

The repository is a Nix flake with a dev shell; with direnv it loads on `cd`.

```sh
nix develop                      # or: direnv allow
cargo run -p spectraplot-gui
```

Pick the spectractrl port in **Port**, set up a scan in the planner on the left
and press **start scan**.

### Command line

```
spectraplot [--port DEVICE] [FILE]
```

| option          | meaning                                                   |
|-----------------|-----------------------------------------------------------|
| `--port DEVICE` | connect to this port instead of the remembered one        |
| `FILE`          | open a `.dat` file or `tio` log at start                  |
| `RUST_LOG=…`    | logging via env_logger, e.g. `RUST_LOG=rfd=debug`         |

## Using it

### Planner

| field              | meaning                                                         |
|--------------------|-----------------------------------------------------------------|
| settings…          | presets by name, then recently used settings                    |
| name               | optional; names a preset and the suggested file name            |
| grating            | see the table below                                             |
| start / end / step | nm with exactly one decimal                                     |
| HV                 | PMT voltage, 0–1000 V                                           |
| integration        | PMT integration time per step, 1–60 000 ms                      |

| grating | lines/mm | blaze  | best range   |
|---------|----------|--------|--------------|
| G1      | 600      | 300 nm | 200–450 nm   |
| G2      | 600      | 750 nm | 500–1100 nm  |
| G3      | 1200     | 500 nm | 330–750 nm   |

The R928 PMT covers 185–900 nm. Problems that would make the drive misbehave
block the start (red); leaving a grating's best range or the PMT range is only
a warning (yellow). Below the fields the planner shows the number of steps, the
pure integration time and the exact command it will send, for example

```
3 200.0 201.0 0.1 450 800 SCAN
```

Scans with at least 10 minutes of integration time ask once before they start.
A running scan can be stopped with **abort scan**.

### Plot

| action        | effect                       |
|---------------|------------------------------|
| hover         | wavelength and reading       |
| drag / scroll | pan                          |
| Ctrl + scroll | zoom                         |
| right drag    | zoom into a box              |
| double click  | back to the planned range    |

### Console

F12 drops the console down from the top. It shows every line the device sends,
cleaned of terminal escape codes. Enter sends a command, Up and Down walk
through the last 50. **Help › Device commands** asks the firmware for its own
command list. Commands go to the same port as the scans.

### Files

| file   | content                                                          |
|--------|------------------------------------------------------------------|
| `.dat` | native format: `# <header>` from the device, then `nm,reading,…` |
| `.csv` | export with column names: `nm,read1,…,mean`                      |
| `.svg` | export of the mean as a plot                                     |

New scans suggest `<name>_<YYYYmmdd-HHMMSS>` or, unnamed,
`<start>-<end>_<step>_<volts>_<ms>_<YYYYmmdd-HHMMSS>`. The `.dat` format is
plain text and plots directly with gnuplot (`set datafile separator ","`).

State lives in `$XDG_STATE_HOME/spectraplot` (usually `~/.local/state/spectraplot`):

| file      | content                                     |
|-----------|---------------------------------------------|
| `recent`  | recently opened files, one path per line    |
| `plans`   | recently used settings, one command per line |
| `presets` | named settings, `<command> # <name>`        |
| `port`    | the serial port to reconnect to             |

## Device protocol

spectraplot expects the line protocol of spectractrl:

| direction | line                                                  |
|-----------|-------------------------------------------------------|
| out       | `<grating> <start> <end> <step> <volts> <ms> SCAN`    |
| out       | `abort scan`                                          |
| in        | `[scan:out] # <header>`                               |
| in        | `[scan:out] <nm>,<reading>[,<reading>…]`              |
| in        | `[scan:done] …`, `[scan:fail] …`, `[scan:abort] …`    |

Lines end in `\r`. The console framing of the firmware (`\r ESC[K` before each
event, the redrawn `> ` prompt) is stripped before parsing.

## Measuring tips

- **Open the shutter.** A closed detector shutter gives a flat, noisy baseline
  and no lines at all.
- **Equal slits.** The wider slit sets the resolution, the narrower one the
  signal; unequal slits only produce flat-topped peaks.
- **Higher orders.** Above about twice the shortest wavelength the source
  emits, second-order light appears (at 1080 nm you see the 540 nm line). A
  long-pass filter such as GG395 or OG550 in front of the entrance slit
  removes it.
- **Calibration.** Each grating has its own offset. Neon lines like 540.06,
  703.24, 743.89 and 748.89 nm make good references.

## Troubleshooting on NixOS

| symptom                                         | cause and fix                                                                                     |
|-------------------------------------------------|---------------------------------------------------------------------------------------------------|
| `NoWaylandLib` at start                         | `wayland`, `libxkbcommon`, `vulkan-loader`, `libGL` missing from the dev shell's `LD_LIBRARY_PATH` |
| `RequestAdapterError`, no GPU found             | dev shell nixpkgs older than the system's; update the flake input, then `cargo clean -p spectraplot-gui` |
| file dialogs do nothing                         | `dbus` missing from `LD_LIBRARY_PATH`, or no xdg-desktop-portal with a FileChooser backend        |
| Port menu empty                                 | only USB serial ports are listed; check `ls /dev/ttyACM*`                                         |
| plot stays empty during a scan                  | look at the status line and the console (F12) for what the device actually sends                 |

## Project layout

```
lib/   spectraplot      device-independent logic, fully unit tested
  event.rs              classifies device lines
  sample.rs scan.rs     readings and recorded scans, .dat and .csv
  plan.rs presets.rs    scan settings, validation, command and header format
  history.rs            most-recently-used lists
  svg.rs                plot export
gui/   spectraplot-gui  the egui application, binary `spectraplot`
  main.rs               app state and wiring
  planner.rs console.rs menu.rs help.rs
  device.rs             serial connection and reader thread
  document.rs files.rs  the open scan and everything on disk
```

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
```

Tests are table driven; logic that can live in `lib/` lives there.

## License

See [LICENSE](LICENSE).
