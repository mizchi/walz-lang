//! CLI transport for structured diagnostics and read-only type queries.
use std::{ffi::OsString, path::Path, process::ExitCode};
use walz::{bindings::BindingPolicy, diagnostic::Diagnostic};

pub fn failure(schema: &str, error: &(dyn std::error::Error + 'static)) -> ExitCode {
    let diagnostic = if let Some(error) = error.downcast_ref::<walz::Error>() {
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

pub fn check(
    path: &Path,
    policy: &BindingPolicy,
    legacy: bool,
    json: bool,
    performance: bool,
) -> ExitCode {
    let result = (|| -> Result<Option<walz::performance::Report>, Box<dyn std::error::Error>> {
        let loaded = if legacy {
            walz::source::load_legacy(path)?
        } else {
            walz::source::load(path)?
        };
        if performance {
            let report = walz::performance::check_with_policy(loaded.text(), policy)
                .map_err(|e| loaded.diagnostic(e))?
                .located(&loaded);
            return Ok(Some(report));
        } else if policy.imports.is_empty() {
            walz::validate(loaded.text()).map_err(|e| loaded.diagnostic(e))?;
        } else {
            // Compile with the policy so laws and all existing contracts are
            // checked as well; no host code is executed.
            walz::compile_with_policy(loaded.text(), policy).map_err(|e| loaded.diagnostic(e))?;
        }
        Ok(None)
    })();
    match result {
        Ok(report) => {
            if json {
                let mut output =
                    serde_json::json!({"schema":"walz-check-v1","ok":true,"diagnostics":[]});
                if let Some(report) = report {
                    output["performance"] = serde_json::to_value(report).unwrap();
                }
                println!("{output}");
            } else if let Some(report) = report {
                for warning in report.warnings {
                    eprintln!("warning[{}]: {warning}", warning.code);
                }
                if report.coverage == "opaque_wat" {
                    eprintln!(
                        "performance advice unavailable: native WAT has no typed expression IR"
                    );
                }
                println!("ok");
            }
            ExitCode::SUCCESS
        }
        Err(error) if json => failure("walz-check-v1", error.as_ref()),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

pub fn type_at(args: &[OsString], policy: &BindingPolicy) -> ExitCode {
    let invalid = || {
        eprintln!("Usage: walz type-at <file.walz> <line> <column> [--source <file.walz>]");
        ExitCode::from(2)
    };
    if !matches!(args.len(), 3 | 5) || (args.len() == 5 && args[3] != "--source") {
        return invalid();
    }
    let coordinate = |i: usize| args[i].to_str()?.parse::<usize>().ok().filter(|n| *n > 0);
    let (Some(line), Some(column)) = (coordinate(1), coordinate(2)) else {
        return invalid();
    };
    let result = walz::source::load(Path::new(&args[0])).and_then(|source| {
        walz::inspect::type_at(&source, args.get(4).map(Path::new), line, column, policy)
    });
    match result {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string(&report).expect("serializable type query")
            );
            ExitCode::SUCCESS
        }
        Err(error) => failure("walz-type-at-v1", &error),
    }
}
