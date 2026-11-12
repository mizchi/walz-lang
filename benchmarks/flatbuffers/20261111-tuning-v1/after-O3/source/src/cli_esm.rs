//! Read-only ABI inspection for Content Mappers. No generated files or host execution.
use std::{
    collections::BTreeMap, ffi::OsString, io::Read, path::PathBuf, process::ExitCode, sync::Arc,
};

pub const USAGE: &str = "Usage: walz esm-metadata <file.walz> [--stdin] [--bindings policy.json]\n\nReturn checked Core/JSPI signatures and source locations as JSON.\n--stdin uses an unsaved UTF-8 source buffer without modifying the file.";
const SCHEMA: &str = "walz-esm-metadata-v1";

pub fn command(args: &[OsString]) -> ExitCode {
    let mut path = None;
    let mut stdin = false;
    let mut bindings = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--stdin") if !stdin => stdin = true,
            Some("--bindings") if bindings.is_none() => {
                bindings = args.next().map(PathBuf::from);
                if bindings.is_none() {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            }
            Some(s) if !s.starts_with('-') && path.is_none() => path = Some(PathBuf::from(arg)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let policy = bindings
            .as_ref()
            .map(|p| {
                walz::bindings::BindingPolicy::parse(&std::fs::read_to_string(p)?)
                    .map_err(Into::<Box<dyn std::error::Error>>::into)
            })
            .transpose()?
            .unwrap_or_default();
        let mut overlays = BTreeMap::new();
        if stdin {
            let mut content = String::new();
            std::io::stdin()
                .take(16 * 1024 * 1024 + 1)
                .read_to_string(&mut content)?;
            if content.len() > 16 * 1024 * 1024 {
                return Err("source exceeds 16 MiB".into());
            }
            overlays.insert(path.clone(), Arc::<str>::from(content));
        }
        let loaded = walz::source::load_with_overlays(&path, &overlays)?;
        let esm = walz::inspect::esm(&loaded, &policy)?;
        let files = loaded
            .paths()
            .map(|p| p.to_owned())
            .chain(bindings.map(|p| p.canonicalize()).transpose()?)
            .collect::<std::collections::BTreeSet<_>>();
        Ok(serde_json::json!({"schema":SCHEMA,"ok":true,"esm":esm,"sourceFiles":files}))
    })();
    match result {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => super::cli_inspection::failure(SCHEMA, error.as_ref()),
    }
}
