//! CLI transport for structured diagnostics and read-only type queries.
use std::{ffi::OsString, path::Path, process::ExitCode};
use wlt::{bindings::BindingPolicy, diagnostic::Diagnostic};

pub fn failure(schema: &str, error: &(dyn std::error::Error + 'static)) -> ExitCode {
    let diagnostic = if let Some(error) = error.downcast_ref::<wlt::Error>() {
        error.diagnostic().clone()
    } else {
        Diagnostic {
            code: "E_TOOL",
            message: error.to_string(),
            primary: None,
            related: vec![],
            notes: vec![],
            help: vec![],
        }
    };
    println!(
        "{}",
        serde_json::json!({"schema":schema,"ok":false,"diagnostics":[diagnostic]})
    );
    ExitCode::FAILURE
}

pub fn check(path: &Path, policy: &BindingPolicy, legacy: bool) -> ExitCode {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let loaded = if legacy {
            wlt::source::load_legacy(path)?
        } else {
            wlt::source::load(path)?
        };
        if policy.imports.is_empty() {
            wlt::validate(loaded.text()).map_err(|e| loaded.diagnostic(e))?;
        } else {
            // Compile with the policy so laws and all existing contracts are
            // checked as well; no host code is executed.
            wlt::compile_with_policy(loaded.text(), policy).map_err(|e| loaded.diagnostic(e))?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            println!(
                "{}",
                serde_json::json!({"schema":"wlt-check-v1","ok":true,"diagnostics":[]})
            );
            ExitCode::SUCCESS
        }
        Err(error) => failure("wlt-check-v1", error.as_ref()),
    }
}

pub fn type_at(args: &[OsString], policy: &BindingPolicy) -> ExitCode {
    let invalid = || {
        eprintln!("Usage: wlt type-at <file.wlt> <line> <column> [--source <file.wlt>]");
        ExitCode::from(2)
    };
    if !matches!(args.len(), 3 | 5) || (args.len() == 5 && args[3] != "--source") {
        return invalid();
    }
    let coordinate = |i: usize| args[i].to_str()?.parse::<usize>().ok().filter(|n| *n > 0);
    let (Some(line), Some(column)) = (coordinate(1), coordinate(2)) else {
        return invalid();
    };
    let result = wlt::source::load(Path::new(&args[0])).and_then(|source| {
        wlt::inspect::type_at(&source, args.get(4).map(Path::new), line, column, policy)
    });
    match result {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string(&report).expect("serializable type query")
            );
            ExitCode::SUCCESS
        }
        Err(error) => failure("wlt-type-at-v1", &error),
    }
}
