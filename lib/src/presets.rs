use crate::plan::ScanPlan;

/// Named scan settings, sorted by name, one per name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Presets {
    pub items: Vec<ScanPlan>,
}

impl Presets {
    /// Reads one plan line per line; unnamed or unreadable lines are skipped.
    pub fn parse(text: &str) -> Self {
        let mut presets = Self::default();
        for plan in text.lines().filter_map(|l| ScanPlan::from_line(l.trim())) {
            presets.save(plan);
        }
        presets
    }

    /// Writes one plan line per preset.
    pub fn to_text(&self) -> String {
        self.items
            .iter()
            .map(|plan| plan.to_line() + "\n")
            .collect()
    }

    /// Stores `plan` under its name, replacing a preset of the same name.
    pub fn save(&mut self, plan: ScanPlan) {
        if plan.name.trim().is_empty() {
            return;
        }
        self.remove(&plan.name);
        self.items.push(plan);
        self.items.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Forgets the preset called `name`.
    pub fn remove(&mut self, name: &str) {
        self.items.retain(|plan| plan.name != name);
    }

    /// True if a preset is called `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.items.iter().any(|plan| plan.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, hv: u16) -> ScanPlan {
        ScanPlan {
            name: name.into(),
            hv,
            ..ScanPlan::default()
        }
    }

    fn presets(items: &[ScanPlan]) -> Presets {
        Presets {
            items: items.to_vec(),
        }
    }

    #[test]
    fn save() {
        let cases = [
            (
                "into empty",
                presets(&[]),
                named("b", 750),
                presets(&[named("b", 750)]),
            ),
            (
                "sorted",
                presets(&[named("b", 750)]),
                named("a", 750),
                presets(&[named("a", 750), named("b", 750)]),
            ),
            (
                "replaces same name",
                presets(&[named("a", 750)]),
                named("a", 900),
                presets(&[named("a", 900)]),
            ),
            (
                "ignores unnamed",
                presets(&[]),
                named(" ", 750),
                presets(&[]),
            ),
        ];
        for (name, mut presets, plan, want) in cases {
            presets.save(plan);
            assert_eq!(presets, want, "{name}");
        }
    }

    #[test]
    fn remove() {
        let mut presets = presets(&[named("a", 750), named("b", 750)]);
        presets.remove("a");
        assert_eq!(presets, self::presets(&[named("b", 750)]));
        assert!(!presets.contains("a"));
        assert!(presets.contains("b"));
    }

    #[test]
    fn roundtrip() {
        let presets = presets(&[named("Ne green", 900), named("Ne red", 750)]);
        assert_eq!(Presets::parse(&presets.to_text()), presets);
    }

    #[test]
    fn parse_skips_unnamed() {
        let text =
            "2 700.0 750.0 0.1 750 3 range-scan\n2 700.0 750.0 0.1 750 3 range-scan # kept\n";
        assert_eq!(Presets::parse(text), presets(&[named("kept", 750)]));
    }
}
