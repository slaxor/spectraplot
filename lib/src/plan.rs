use std::fmt;
use std::ops::RangeInclusive;

/// One grating of the SpectraPro-275 turret.
#[derive(Debug, Clone, PartialEq)]
pub struct Grating {
    pub id: u8,
    pub lines_per_mm: u32,
    pub blaze_nm: f64,
    pub optimal_nm: RangeInclusive<f64>,
}

impl fmt::Display for Grating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "G{} · {} l/mm · blaze {} nm · {}–{} nm",
            self.id,
            self.lines_per_mm,
            self.blaze_nm,
            self.optimal_nm.start(),
            self.optimal_nm.end()
        )
    }
}

/// The installed gratings, as on the sticker.
pub const GRATINGS: [Grating; 3] = [
    Grating {
        id: 1,
        lines_per_mm: 600,
        blaze_nm: 300.0,
        optimal_nm: 200.0..=450.0,
    },
    Grating {
        id: 2,
        lines_per_mm: 600,
        blaze_nm: 750.0,
        optimal_nm: 500.0..=1100.0,
    },
    Grating {
        id: 3,
        lines_per_mm: 1200,
        blaze_nm: 500.0,
        optimal_nm: 330.0..=750.0,
    },
];

/// Wavelengths the drive can reach at all; beyond 1500 nm it runs into the end stop.
pub const DRIVE_NM: RangeInclusive<f64> = 0.0..=1500.0;

/// Spectral range of the R928 PMT.
pub const PMT_NM: RangeInclusive<f64> = 185.0..=900.0;

/// PMT high voltage the planner allows.
pub const HV_VOLTS: RangeInclusive<u16> = 0..=1000;

/// Something wrong or questionable about a plan.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    UnknownGrating(u8),
    EmptyRange,
    StepNotPositive,
    NotOneDecimal(&'static str),
    OutsideDrive,
    HvOutOfRange,
    NoReads,
    OutsideGratingOptimum(u8),
    OutsidePmtRange,
}

impl Problem {
    /// Blocking problems prevent the scan; the others are only warnings.
    pub fn is_blocking(&self) -> bool {
        !matches!(self, Self::OutsideGratingOptimum(_) | Self::OutsidePmtRange)
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownGrating(id) => write!(f, "there is no grating {id}"),
            Self::EmptyRange => write!(f, "start must be below end"),
            Self::StepNotPositive => write!(f, "step must be positive"),
            Self::NotOneDecimal(field) => write!(f, "{field} must have exactly one decimal place"),
            Self::OutsideDrive => {
                write!(
                    f,
                    "range leaves the drive's {}–{} nm",
                    DRIVE_NM.start(),
                    DRIVE_NM.end()
                )
            }
            Self::HvOutOfRange => write!(
                f,
                "HV must be within {}–{} V",
                HV_VOLTS.start(),
                HV_VOLTS.end()
            ),
            Self::NoReads => write!(f, "at least one read per step"),
            Self::OutsideGratingOptimum(id) => {
                write!(f, "range leaves the optimum of grating {id}")
            }
            Self::OutsidePmtRange => {
                write!(
                    f,
                    "range leaves the PMT's {}–{} nm",
                    PMT_NM.start(),
                    PMT_NM.end()
                )
            }
        }
    }
}

/// Parameters of one `range-scan`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanPlan {
    pub name: String,
    pub grating: u8,
    pub start_nm: f64,
    pub end_nm: f64,
    pub step_nm: f64,
    pub hv: u16,
    pub reads: u8,
}

impl Default for ScanPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            grating: 2,
            start_nm: 700.0,
            end_nm: 750.0,
            step_nm: 0.1,
            hv: 750,
            reads: 3,
        }
    }
}

impl ScanPlan {
    /// The grating this plan selects, if it exists.
    pub fn grating(&self) -> Option<&'static Grating> {
        GRATINGS.iter().find(|g| g.id == self.grating)
    }

    /// Everything wrong with the plan, blocking problems and warnings alike.
    pub fn problems(&self) -> Vec<Problem> {
        let mut found = Vec::new();
        match self.grating() {
            None => found.push(Problem::UnknownGrating(self.grating)),
            Some(g) if !self.lies_within(&g.optimal_nm) => {
                found.push(Problem::OutsideGratingOptimum(g.id))
            }
            Some(_) => {}
        }
        if self.start_nm >= self.end_nm {
            found.push(Problem::EmptyRange);
        }
        if self.step_nm <= 0.0 {
            found.push(Problem::StepNotPositive);
        }
        for (field, value) in [
            ("start", self.start_nm),
            ("end", self.end_nm),
            ("step", self.step_nm),
        ] {
            if !has_one_decimal(value) {
                found.push(Problem::NotOneDecimal(field));
            }
        }
        if !self.lies_within(&DRIVE_NM) {
            found.push(Problem::OutsideDrive);
        }
        if !self.lies_within(&PMT_NM) {
            found.push(Problem::OutsidePmtRange);
        }
        if !HV_VOLTS.contains(&self.hv) {
            found.push(Problem::HvOutOfRange);
        }
        if self.reads == 0 {
            found.push(Problem::NoReads);
        }
        found
    }

    /// True when no blocking problem exists.
    pub fn is_runnable(&self) -> bool {
        !self.problems().iter().any(Problem::is_blocking)
    }

    /// Number of grating positions the scan visits, both ends included.
    pub fn steps(&self) -> usize {
        if self.step_nm <= 0.0 || self.start_nm > self.end_nm {
            return 0;
        }
        ((self.end_nm - self.start_nm) / self.step_nm).round() as usize + 1
    }

    /// The spectractrl command line, formatted the way the drive accepts it.
    pub fn to_command(&self) -> String {
        format!(
            "{} {:.1} {:.1} {:.1} {} {} range-scan",
            self.grating, self.start_nm, self.end_nm, self.step_nm, self.hv, self.reads
        )
    }

    fn lies_within(&self, range: &RangeInclusive<f64>) -> bool {
        range.contains(&self.start_nm) && range.contains(&self.end_nm)
    }
}

fn has_one_decimal(value: f64) -> bool {
    let tenths = value * 10.0;
    (tenths - tenths.round()).abs() < 1e-6
}

impl ScanPlan {
    /// Reads back a line written by [`ScanPlan::to_command`].
    pub fn from_command(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [grating, start, end, step, hv, reads, word] = fields[..] else {
            return None;
        };
        if !word.eq_ignore_ascii_case("range-scan") {
            return None;
        }
        Some(Self {
            name: String::new(),
            grating: grating.parse().ok()?,
            start_nm: start.parse().ok()?,
            end_nm: end.parse().ok()?,
            step_nm: step.parse().ok()?,
            hv: hv.parse().ok()?,
            reads: reads.parse().ok()?,
        })
    }

    /// Reads the plan from a scan header such as
    /// `grating 2, 700.0..750.0 nm, step 0.1 nm, 750 V, 3 reads per step`.
    ///
    /// Point measurements and scans without a set voltage yield `None`.
    pub fn from_header(header: &str) -> Option<Self> {
        let mut parts = header.split(", ");
        let grating = parts.next()?.strip_prefix("grating ")?;
        let (start, end) = parts.next()?.strip_suffix(" nm")?.split_once("..")?;
        let step = parts.next()?.strip_prefix("step ")?.strip_suffix(" nm")?;
        let hv = parts.next()?.strip_suffix(" V")?;
        let reads = parts.next()?.strip_suffix(" reads per step")?;
        Some(Self {
            name: String::new(),
            grating: grating.parse().ok()?,
            start_nm: start.parse().ok()?,
            end_nm: end.parse().ok()?,
            step_nm: step.parse().ok()?,
            hv: hv.parse().ok()?,
            reads: reads.parse().ok()?,
        })
    }

    /// The storage line: the command, followed by ` # name` when the plan has one.
    pub fn to_line(&self) -> String {
        if self.name.is_empty() {
            self.to_command()
        } else {
            format!("{} # {}", self.to_command(), self.name)
        }
    }

    /// Reads back a line written by [`ScanPlan::to_line`].
    pub fn from_line(line: &str) -> Option<Self> {
        let (command, name) = line.split_once(" # ").unwrap_or((line, ""));
        let plan = Self::from_command(command)?;
        Some(Self {
            name: name.trim().to_string(),
            ..plan
        })
    }

    /// A file name stem: the sanitised name, or the parameters when unnamed.
    pub fn file_stem(&self) -> String {
        let name = self.name.trim();
        if !name.is_empty() {
            return sanitise(name);
        }
        format!(
            "{:.1}-{:.1}_{:.1}_{}_{}",
            self.start_nm, self.end_nm, self.step_nm, self.hv, self.reads
        )
    }

    /// The same plan with wavelengths rounded to the 0.1 nm the drive accepts.
    pub fn rounded(&self) -> Self {
        Self {
            start_nm: to_tenth(self.start_nm),
            end_nm: to_tenth(self.end_nm),
            step_nm: to_tenth(self.step_nm),
            ..self.clone()
        }
    }
}

fn sanitise(name: &str) -> String {
    name.chars()
        .map(|c| if is_file_safe(c) { c } else { '_' })
        .collect()
}

fn is_file_safe(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '-' | '_')
}

fn to_tenth(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(change: impl FnOnce(&mut ScanPlan)) -> ScanPlan {
        let mut plan = ScanPlan::default();
        change(&mut plan);
        plan
    }

    #[test]
    fn problems() {
        let cases = [
            ("default", plan(|_| {}), vec![]),
            (
                "unknown grating",
                plan(|p| p.grating = 4),
                vec![Problem::UnknownGrating(4)],
            ),
            (
                "empty range",
                plan(|p| p.end_nm = 700.0),
                vec![Problem::EmptyRange],
            ),
            (
                "zero step",
                plan(|p| p.step_nm = 0.0),
                vec![Problem::StepNotPositive],
            ),
            (
                "two decimals",
                plan(|p| p.start_nm = 700.05),
                vec![Problem::NotOneDecimal("start")],
            ),
            (
                "beyond drive",
                plan(|p| p.end_nm = 1600.0),
                vec![
                    Problem::OutsideGratingOptimum(2),
                    Problem::OutsideDrive,
                    Problem::OutsidePmtRange,
                ],
            ),
            (
                "hv too high",
                plan(|p| p.hv = 1200),
                vec![Problem::HvOutOfRange],
            ),
            ("no reads", plan(|p| p.reads = 0), vec![Problem::NoReads]),
            (
                "outside optimum",
                plan(|p| p.grating = 1),
                vec![Problem::OutsideGratingOptimum(1)],
            ),
        ];
        for (name, plan, want) in cases {
            assert_eq!(plan.problems(), want, "{name}");
        }
    }

    #[test]
    fn is_runnable() {
        let cases = [
            ("default", plan(|_| {}), true),
            ("warning only", plan(|p| p.grating = 1), true),
            ("blocking", plan(|p| p.reads = 0), false),
        ];
        for (name, plan, want) in cases {
            assert_eq!(plan.is_runnable(), want, "{name}");
        }
    }

    #[test]
    fn steps() {
        let cases = [
            ("today's scan", plan(|_| {}), 501),
            ("single position", plan(|p| p.end_nm = p.start_nm), 1),
            ("coarse", plan(|p| p.step_nm = 10.0), 6),
            ("zero step", plan(|p| p.step_nm = 0.0), 0),
            ("reversed", plan(|p| p.end_nm = 600.0), 0),
        ];
        for (name, plan, want) in cases {
            assert_eq!(plan.steps(), want, "{name}");
        }
    }

    #[test]
    fn to_command() {
        let cases = [
            (
                "today's scan",
                plan(|_| {}),
                "2 700.0 750.0 0.1 750 3 range-scan",
            ),
            (
                "integral values",
                plan(|p| {
                    p.grating = 3;
                    p.start_nm = 400.0;
                    p.end_nm = 500.0;
                    p.step_nm = 1.0;
                }),
                "3 400.0 500.0 1.0 750 3 range-scan",
            ),
            (
                "float noise",
                plan(|p| p.start_nm = 700.1 + 0.2 - 0.2),
                "2 700.1 750.0 0.1 750 3 range-scan",
            ),
        ];
        for (name, plan, want) in cases {
            assert_eq!(plan.to_command(), want, "{name}");
        }
    }

    #[test]
    fn from_command() {
        let cases = [
            (
                "own output",
                ScanPlan::default().to_command(),
                Some(ScanPlan::default()),
            ),
            (
                "upper case",
                "2 700.0 750.0 0.1 750 3 RANGE-SCAN".into(),
                Some(ScanPlan::default()),
            ),
            ("other macro", "2 700.0 3 POINT-MEASURE".into(), None),
            (
                "missing field",
                "2 700.0 750.0 0.1 750 range-scan".into(),
                None,
            ),
            (
                "bad number",
                "2 700.0 750.0 0.1 x 3 range-scan".into(),
                None,
            ),
            ("empty", String::new(), None),
        ];
        for (name, line, want) in cases {
            assert_eq!(ScanPlan::from_command(&line), want, "{name}");
        }
    }

    #[test]
    fn from_header() {
        let cases = [
            (
                "range scan",
                "grating 2, 700.0..750.0 nm, step 0.1 nm, 750 V, 3 reads per step",
                Some(ScanPlan::default()),
            ),
            (
                "point measure",
                "grating 1, at 300.5 nm, 750 V, 5 reads per step",
                None,
            ),
            (
                "voltage unchanged",
                "grating 2, 700.0..750.0 nm, step 0.1 nm, PMT voltage unchanged, 3 reads per step",
                None,
            ),
            ("junk", "hello", None),
        ];
        for (name, header, want) in cases {
            assert_eq!(ScanPlan::from_header(header), want, "{name}");
        }
    }

    #[test]
    fn rounded() {
        let drifted = plan(|p| {
            p.start_nm = 700.1 + 0.2;
            p.step_nm = 0.30000000000000004;
        });
        let want = plan(|p| {
            p.start_nm = 700.3;
            p.step_nm = 0.3;
        });
        assert_eq!(drifted.rounded(), want);
    }

    #[test]
    fn line_roundtrip() {
        let named = plan(|p| p.name = "Ne green line fine resolution".into());
        let cases = [
            (
                "unnamed",
                plan(|_| {}),
                "2 700.0 750.0 0.1 750 3 range-scan",
            ),
            (
                "named",
                named,
                "2 700.0 750.0 0.1 750 3 range-scan # Ne green line fine resolution",
            ),
        ];
        for (name, plan, want) in cases {
            assert_eq!(plan.to_line(), want, "{name}");
            assert_eq!(ScanPlan::from_line(want), Some(plan), "{name} roundtrip");
        }
    }

    #[test]
    fn file_stem() {
        let cases = [
            ("unnamed", plan(|_| {}), "700.0-750.0_0.1_750_3"),
            (
                "named",
                plan(|p| p.name = "Ne green line fine resolution".into()),
                "Ne_green_line_fine_resolution",
            ),
            (
                "unsafe characters",
                plan(|p| p.name = " a/b:c ".into()),
                "a_b_c",
            ),
            (
                "blank name",
                plan(|p| p.name = "  ".into()),
                "700.0-750.0_0.1_750_3",
            ),
        ];
        for (name, plan, want) in cases {
            assert_eq!(plan.file_stem(), want, "{name}");
        }
    }
}
