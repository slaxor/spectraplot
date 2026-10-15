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
}
