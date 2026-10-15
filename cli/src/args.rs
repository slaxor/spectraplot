/// Command line: `specplot [--port DEVICE] [CSV]`.
#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub port: Option<String>,
    pub csv: Option<String>,
}

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => parsed.port = Some(args.next().ok_or("--port needs a device")?),
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}")),
            _ if parsed.csv.is_none() => parsed.csv = Some(arg),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(port: Option<&str>, csv: Option<&str>) -> Args {
        Args {
            port: port.map(Into::into),
            csv: csv.map(Into::into),
        }
    }

    #[test]
    fn parse() {
        let cases = [
            ("nothing", vec![], Ok(args(None, None))),
            ("csv", vec!["scan.csv"], Ok(args(None, Some("scan.csv")))),
            (
                "port",
                vec!["--port", "/dev/ttyACM0"],
                Ok(args(Some("/dev/ttyACM0"), None)),
            ),
            (
                "both",
                vec!["scan.csv", "--port", "/dev/ttyACM0"],
                Ok(args(Some("/dev/ttyACM0"), Some("scan.csv"))),
            ),
            (
                "port without device",
                vec!["--port"],
                Err("--port needs a device".into()),
            ),
            (
                "unknown option",
                vec!["-v"],
                Err("unknown option -v".into()),
            ),
            (
                "two csvs",
                vec!["a.csv", "b.csv"],
                Err("unexpected argument b.csv".into()),
            ),
        ];
        for (name, argv, want) in cases {
            assert_eq!(
                super::parse(argv.into_iter().map(String::from)),
                want,
                "{name}"
            );
        }
    }
}
