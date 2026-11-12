use std::{ffi::OsString, path::PathBuf, process::ExitCode};

pub const USAGE: &str = "Usage: walz print [--format walz|watz] <file>\n\nPrint source without expanding imports/macros, checking, or modifying files.\nDefault: Walz S-expressions. --format watz selects the optional review view.\n.watz input can be printed back to Walz. -- ends option parsing.";

#[derive(Default)]
enum Format {
    #[default]
    Walz,
    Watz,
}

fn set_format(slot: &mut Option<Format>, value: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err("duplicate --format".into());
    }
    *slot = Some(match value {
        "walz" => Format::Walz,
        "watz" => Format::Watz,
        _ => return Err("--format requires walz or watz".into()),
    });
    Ok(())
}

fn arguments(args: &[OsString]) -> Result<(PathBuf, Format), String> {
    let mut path = None;
    let mut selected = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--format") => {
                let value = args
                    .next()
                    .and_then(|s| s.to_str())
                    .ok_or("--format requires walz or watz")?;
                set_format(&mut selected, value)?;
            }
            Some(flag) if flag.starts_with("--format=") => {
                set_format(&mut selected, &flag["--format=".len()..])?;
            }
            Some("--") => {
                for arg in args {
                    if path.replace(PathBuf::from(arg)).is_some() {
                        return Err("expected one input file".into());
                    }
                }
                break;
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown print option {flag:?}"));
            }
            _ => {
                if path.replace(PathBuf::from(arg)).is_some() {
                    return Err("expected one input file".into());
                }
            }
        }
    }
    Ok((
        path.ok_or("expected one input file")?,
        selected.unwrap_or_default(),
    ))
}

pub fn command(args: &[OsString]) -> ExitCode {
    let (path, selected) = match arguments(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        let original = std::fs::read_to_string(&path)?;
        let source = if walz::review::is_source_path(&path) {
            walz::review::from_watz(&original)?.text
        } else {
            original
        };
        match selected {
            Format::Walz => walz::format::source(&source),
            Format::Watz => walz::review::to_watz(&source),
        }
        .map_err(|e| Box::new(e) as _)
    })();
    match result {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            ExitCode::FAILURE
        }
    }
}
