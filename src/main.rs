use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

mod cli_inspection;

const USAGE: &str = "Usage: wlt <check|types|wat|expand|wit|analyze|ir|contracts|proof|verify|run|run-verified|component-wat|fmt|fmt-check> <file.wlt>\n       wlt check --json <file.wlt>\n       wlt type-at <file.wlt> <line> <column> [--source <file.wlt>]\n       wlt <build|component> <file.wlt> <output.wasm>\n       wlt <core-js|profile-text> <file.wlt> <output-directory>\n       wlt test [paths...] [--filter text] [--list] [--timeout-ms N]";

fn test_arguments(args: &[OsString]) -> Result<(Vec<PathBuf>, wlt::runner::RunOptions), String> {
    let mut paths = Vec::new();
    let mut options = wlt::runner::RunOptions::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--list") => options.list = true,
            Some("--filter") => {
                options.filter = Some(
                    args.next()
                        .and_then(|s| s.to_str())
                        .ok_or("--filter requires UTF-8 text")?
                        .to_owned(),
                );
            }
            Some("--timeout-ms") => {
                let value = args
                    .next()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .filter(|&value| value > 0)
                    .ok_or("--timeout-ms requires a positive integer")?;
                options.timeout = Duration::from_millis(value);
            }
            Some("--") => {
                paths.extend(args.map(PathBuf::from));
                break;
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown test option {flag:?}"));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    Ok((paths, options))
}

fn test_command(args: &[OsString], legacy: bool) -> ExitCode {
    use wlt::runner::TestOutcome;
    let (paths, mut options) = match test_arguments(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    options.legacy_syntax = legacy;
    let report = wlt::runner::run(&paths, &options);
    for test in &report.tests {
        let path = test.path.display();
        match &test.outcome {
            TestOutcome::Listed => println!("{path}::{:?}: test", test.name),
            TestOutcome::Passed => println!("test {path}::{:?} ... ok", test.name),
            TestOutcome::Failed(message) => {
                println!("test {path}::{:?} ... FAILED\n  {message}", test.name)
            }
            TestOutcome::TimedOut => println!(
                "test {path}::{:?} ... FAILED\n  timed out after {} ms",
                test.name,
                options.timeout.as_millis()
            ),
        }
    }
    for error in &report.errors {
        let path = error
            .path
            .as_ref()
            .map(|p| format!("{}: ", p.display()))
            .unwrap_or_default();
        println!("ERROR {path}{}", error.message);
    }
    if options.list {
        println!(
            "{} tests; {} filtered out; {} errors",
            report.tests.len(),
            report.filtered_out,
            report.errors.len()
        );
    } else {
        println!(
            "{} passed; {} failed; {} errors; {} filtered out",
            report.passed(),
            report.failed(),
            report.errors.len(),
            report.filtered_out
        );
    }
    if report.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn execute(
    command: &str,
    path: &Path,
    output: Option<&Path>,
    policy: &wlt::bindings::BindingPolicy,
    legacy: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(command, "fmt" | "fmt-check") {
        let original = std::fs::read_to_string(path)?;
        let formatted =
            wlt::format::source(&original).map_err(|e| format!("{}:{e}", path.display()))?;
        if command == "fmt" {
            if original != formatted {
                std::fs::write(path, formatted)?;
            }
            return Ok(true);
        }
        if original != formatted {
            eprintln!("{}: source is not formatted", path.display());
            return Ok(false);
        }
        return Ok(true);
    }
    let loaded = if legacy {
        wlt::source::load_legacy(path)?
    } else {
        wlt::source::load(path)?
    };
    execute_source(command, path, output, loaded.text(), policy).map_err(|error| {
        match error.downcast::<wlt::Error>() {
            Ok(error) => loaded.diagnostic(*error).into(),
            Err(error) => error,
        }
    })
}

fn execute_source(
    command: &str,
    path: &Path,
    output: Option<&Path>,
    source: &str,
    policy: &wlt::bindings::BindingPolicy,
) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(command, "core-js" | "profile-text") {
        let built = wlt::component::core_js::compile_with_policy(source, policy)?;
        let profile = if command == "profile-text" {
            Some(wlt::component::profiling::compile(source)?)
        } else {
            None
        };
        let output = output.expect("Core output directory");
        let input = std::fs::canonicalize(path)?;
        let mut names = vec!["core.wasm", "memory.wasm", "bindings.mjs", "manifest.json"];
        if profile.is_some() {
            names.extend(["env.wasm", "env.wat", "driver.wasm", "driver.wat"]);
        }
        for name in names {
            if std::fs::canonicalize(output.join(name)).ok().as_ref() == Some(&input) {
                return Err("output must differ from source path".into());
            }
        }
        std::fs::create_dir_all(output)?;
        // All source checks and Wasm validation finish before publishing files.
        std::fs::write(output.join("core.wasm"), built.wasm)?;
        std::fs::write(output.join("memory.wasm"), built.memory)?;
        std::fs::write(output.join("bindings.mjs"), built.bindings)?;
        std::fs::write(
            output.join("manifest.json"),
            serde_json::to_vec_pretty(&built.manifest)?,
        )?;
        if let Some(profile) = profile {
            std::fs::write(output.join("env.wasm"), profile.env)?;
            std::fs::write(output.join("env.wat"), profile.env_wat)?;
            std::fs::write(output.join("driver.wasm"), profile.driver)?;
            std::fs::write(output.join("driver.wat"), profile.driver_wat)?;
        }
        return Ok(true);
    }
    if matches!(command, "component" | "component-wat") {
        let built = wlt::component::compile_with_policy(source, policy)?;
        if command == "component-wat" {
            print!("{}", built.wat);
        } else {
            let output = output.expect("component output");
            if path == output {
                return Err("output must differ from source path".into());
            }
            let mut metadata = output.as_os_str().to_os_string();
            metadata.push(".json");
            let metadata = PathBuf::from(metadata);
            if metadata == path {
                return Err("metadata output must differ from source path".into());
            }
            std::fs::write(output, built.wasm)?;
            std::fs::write(metadata, serde_json::to_vec_pretty(&built.manifest)?)?;
        }
        return Ok(true);
    }
    if command == "proof" {
        print!(
            "{}",
            wlt::proof::emit(&wlt::check_with_policy(source, policy)?)?
        );
        return Ok(true);
    }
    if command == "contracts" {
        let module = wlt::check_with_policy(source, policy)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&wlt::ownership::validate(source, &module)?)?
        );
        return Ok(true);
    }
    if command == "wit" {
        let module = wlt::check_with_policy(source, policy)?;
        print!("{}", wlt::wit::emit(&module)?);
        return Ok(true);
    }
    if command == "check" {
        if policy.imports.is_empty() {
            wlt::validate(source)?;
        } else {
            wlt::check_with_policy(source, policy)?;
        }
        println!("ok");
        return Ok(true);
    }
    // Source inspection depends only on the front end, not on Wasm emission.
    if matches!(command, "check" | "types" | "ir" | "analyze") {
        let module = match wlt::check_with_policy(source, policy) {
            Ok(module) => module,
            Err(_) => {
                let compilation = wlt::compile_with_policy(source, policy)?;
                match command {
                    "types" => println!(
                        "{}",
                        serde_json::to_string_pretty(&wlt::native::types(&compilation.wasm)?)?
                    ),
                    "ir" => println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({ "mode": "wat", "structured_ir": null, "wat": compilation.wat })
                        )?
                    ),
                    "analyze" => {
                        println!("{}", serde_json::to_string_pretty(&compilation.analysis)?)
                    }
                    _ => unreachable!(),
                }
                return Ok(true);
            }
        };
        match command {
            "check" => println!("ok"),
            "types" => println!(
                "{}",
                serde_json::to_string_pretty(&wlt::typing::report(&module))?
            ),
            "ir" => println!("{}", serde_json::to_string_pretty(&module)?),
            "analyze" => println!(
                "{}",
                serde_json::to_string_pretty(&wlt::analysis::analyze(&module))?
            ),
            _ => unreachable!(),
        }
        return Ok(true);
    }
    let compiled = if matches!(command, "verify" | "run-verified") {
        wlt::verified::compile(source)?
    } else {
        wlt::compile_with_policy(source, policy)?
    };
    match command {
        "wat" | "expand" => print!("{}", compiled.wat),
        "build" => {
            let output = output.expect("argument validation requires an output path");
            if path == output {
                return Err("output must differ from source path".into());
            }
            std::fs::write(output, compiled.wasm)?;
        }
        "verify" => println!("ownership contracts and laws verified; Wasm validated"),
        "run" | "run-verified" => {
            let mut file = tempfile::Builder::new().suffix(".wasm").tempfile()?;
            file.write_all(&compiled.wasm)?;
            file.flush()?;
            let status = Command::new("wasmtime")
                .args([
                    "run",
                    "-W",
                    "gc=y,function-references=y,exceptions=y",
                    "--invoke",
                    "main",
                ])
                .arg(file.path())
                .status()
                .map_err(|error| format!("could not start wasmtime: {error}"))?;
            return Ok(status.success());
        }
        _ => unreachable!("command was validated"),
    }
    Ok(true)
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let json = args
        .iter()
        .position(|s| s == "--json")
        .map(|i| args.remove(i))
        .is_some();
    if json && !args.first().is_some_and(|s| s == "check") {
        eprintln!("--json is supported by check; type-at always returns JSON\n{USAGE}");
        return ExitCode::from(2);
    }
    // Compatibility hook for frozen evaluation inputs only.
    let legacy = args
        .iter()
        .position(|s| s == "--legacy-syntax")
        .map(|i| args.remove(i))
        .is_some();
    if args.first().is_some_and(|s| s == "bind") {
        if args.len() != 4 {
            eprintln!("Usage: wlt bind <input.wit> <policy.json> <output-directory>");
            return ExitCode::from(2);
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let generated = wlt::wit_bindgen::generate(
                Path::new(&args[1]),
                None,
                &std::fs::read_to_string(&args[2])?,
            )?;
            let out = Path::new(&args[3]);
            if out.join("bindings.wlt").exists() || out.join("bindings.json").exists() {
                return Err("binding outputs already exist".into());
            }
            std::fs::create_dir_all(out)?;
            std::fs::write(out.join("bindings.wlt"), generated.source)?;
            std::fs::write(out.join("bindings.json"), generated.policy)?;
            Ok(())
        })();
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    let mut policy = wlt::bindings::BindingPolicy::default();
    if let Some(i) = args.iter().position(|s| s == "--bindings") {
        let Some(path) = args.get(i + 1) else {
            eprintln!("--bindings requires a policy file");
            return ExitCode::from(2);
        };
        match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| wlt::bindings::BindingPolicy::parse(&s).map_err(|e| e.to_string()))
        {
            Ok(p) => policy = p,
            Err(e) => {
                if json || args.first().is_some_and(|s| s == "type-at") {
                    return cli_inspection::failure(
                        if json {
                            "wlt-check-v1"
                        } else {
                            "wlt-type-at-v1"
                        },
                        &std::io::Error::other(e),
                    );
                }
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
        args.drain(i..i + 2);
    }
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(command) = args.first().and_then(|arg| arg.to_str()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if command == "test" {
        return test_command(&args[1..], legacy);
    }
    if command == "type-at" {
        return cli_inspection::type_at(&args[1..], &policy);
    }
    if !matches!(
        command,
        "check"
            | "fmt"
            | "fmt-check"
            | "types"
            | "wat"
            | "expand"
            | "wit"
            | "analyze"
            | "ir"
            | "contracts"
            | "verify"
            | "proof"
            | "run"
            | "run-verified"
            | "build"
            | "component"
            | "component-wat"
            | "core-js"
            | "profile-text"
    ) || args.len()
        != if matches!(command, "build" | "component" | "core-js" | "profile-text") {
            3
        } else {
            2
        }
    {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let path = Path::new(&args[1]);
    if json {
        return cli_inspection::check(path, &policy, legacy);
    }
    match execute(command, path, args.get(2).map(Path::new), &policy, legacy) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
