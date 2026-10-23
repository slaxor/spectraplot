/// Command line: `spectraplot [--port DEVICE] [FILE]`.
#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub port: Option<String>,
    pub file: Option<String>,
}

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => parsed.port = Some(args.next().ok_or("--port needs a device")?),
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}")),
            _ if parsed.file.is_none() => parsed.file = Some(arg),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(port: Option<&str>, file: Option<&str>) -> Args {
        Args {
            port: port.map(Into::into),
            file: file.map(Into::into),
        }
    }

    #[test]
    fn parse() {
        let cases = [
            ("nothing", vec![], Ok(args(None, None))),
            ("file", vec!["scan.dat"], Ok(args(None, Some("scan.dat")))),
            (
                "port",
                vec!["--port", "/dev/ttyACM0"],
                Ok(args(Some("/dev/ttyACM0"), None)),
            ),
            (
                "both",
                vec!["scan.dat", "--port", "/dev/ttyACM0"],
                Ok(args(Some("/dev/ttyACM0"), Some("scan.dat"))),
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
                "two files",
                vec!["a.dat", "b.dat"],
                Err("unexpected argument b.dat".into()),
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
