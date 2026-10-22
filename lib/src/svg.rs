use crate::scan::Scan;

const WIDTH: f64 = 1000.0;
const HEIGHT: f64 = 600.0;
const MARGIN: f64 = 60.0;

/// Renders the mean of each step as a line plot.
pub fn render(scan: &Scan) -> String {
    let points: Vec<(f64, f64)> = scan.samples.iter().map(|s| (s.nm, s.mean())).collect();
    let x = Axis::spanning(points.iter().map(|p| p.0));
    let y = Axis::spanning(points.iter().map(|p| p.1));
    let polyline: Vec<String> = points
        .iter()
        .map(|&(nm, value)| {
            format!(
                "{:.1},{:.1}",
                x.to_px(nm, WIDTH),
                HEIGHT - y.to_px(value, HEIGHT)
            )
        })
        .collect();
    let title = escape(scan.header.as_deref().unwrap_or(""));
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" font-family="sans-serif" font-size="14">
<rect width="100%" height="100%" fill="white"/>
<rect x="{MARGIN}" y="{MARGIN}" width="{w}" height="{h}" fill="none" stroke="black"/>
<text x="{cx}" y="{t}" text-anchor="middle">{title}</text>
<text x="{MARGIN}" y="{b}" text-anchor="middle">{x0}</text>
<text x="{r}" y="{b}" text-anchor="middle">{x1}</text>
<text x="{cx}" y="{b}" text-anchor="middle">nm</text>
<text x="{l}" y="{bottom}" text-anchor="end">{y0}</text>
<text x="{l}" y="{top}" text-anchor="end">{y1}</text>
<polyline fill="none" stroke="purple" points="{points}"/>
</svg>
"#,
        w = WIDTH - 2.0 * MARGIN,
        h = HEIGHT - 2.0 * MARGIN,
        cx = WIDTH / 2.0,
        t = MARGIN / 2.0,
        b = HEIGHT - MARGIN / 3.0,
        r = WIDTH - MARGIN,
        l = MARGIN - 5.0,
        top = MARGIN + 5.0,
        bottom = HEIGHT - MARGIN,
        x0 = x.min,
        x1 = x.max,
        y0 = y.min,
        y1 = y.max,
        points = polyline.join(" "),
    )
}

struct Axis {
    min: f64,
    max: f64,
}

impl Axis {
    fn spanning(values: impl Iterator<Item = f64>) -> Self {
        let (min, max) = values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
        if min.is_finite() {
            Self { min, max }
        } else {
            Self { min: 0.0, max: 0.0 }
        }
    }

    fn to_px(&self, value: f64, size: f64) -> f64 {
        let span = self.max - self.min;
        let fraction = if span > 0.0 {
            (value - self.min) / span
        } else {
            0.5
        };
        MARGIN + fraction * (size - 2.0 * MARGIN)
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::Sample;

    fn scan(header: Option<&str>, points: &[(f64, u32)]) -> Scan {
        let samples = points
            .iter()
            .map(|&(nm, r)| Sample { nm, reads: vec![r] })
            .collect();
        Scan {
            header: header.map(Into::into),
            samples,
        }
    }

    #[test]
    fn render() {
        let cases = [
            (
                "corners",
                scan(None, &[(700.0, 0), (710.0, 100)]),
                "points=\"60.0,540.0 940.0,60.0\"",
            ),
            (
                "flat line centred",
                scan(None, &[(700.0, 5), (710.0, 5)]),
                "points=\"60.0,300.0 940.0,300.0\"",
            ),
            ("empty", scan(None, &[]), "points=\"\""),
            (
                "escaped title",
                scan(Some("a<b & c"), &[]),
                ">a&lt;b &amp; c<",
            ),
        ];
        for (name, scan, want) in cases {
            let svg = super::render(&scan);
            assert!(svg.contains(want), "{name}: {svg}");
        }
    }
}
