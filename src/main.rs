use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

mod cli_bench;
mod cli_build;
mod cli_esm;
mod cli_inspection;
mod cli_js;
mod cli_new;
mod cli_print;
mod cli_run;

const USAGE: &str = "Usage: walz <command> [options]\n\n  print [--format walz|watz] <file>             Print Walz or an optional Watz view\n  new <directory>                              Create a Walz application\n  build [file.walz|directory] [-o output] [--target core|core-js|component]\n  test [paths...]                              Run *.test.walz (default: tests/)\n  bench [paths...]                             Run *_bench.walz (default: .)\n\n  walz <check|types|wat|expand|wit|analyze|ir|contracts|race-contracts|proof|verify|run|run-verified|component-wat|fmt|fmt-check> <file.walz>\n  walz check --json <file.walz>\n  walz esm-metadata <file.walz> [--stdin]       Checked JavaScript ABI as JSON\n  walz lsp [--bindings policy.json]            Language Server Protocol over stdio\n  walz macro-expand [--text] <file.walz>                 Checked expansion history as JSON\n  walz type-at <file.walz> <line> <column> [--source <file.walz>]\n  walz component <file.walz> <output.wasm>\n  walz <core-js|profile-text> <file.walz> <output-directory>\n  walz bind <input.wit> <policy.json> <output-directory> [--world name]\n\nUse walz <command> --help for options; walz --version for the version.";
const TEST_USAGE: &str = "Usage: walz test [paths...] [--target core|core-js] [--bindings policy.json] [--host host.mjs] [--filter text] [--list] [--timeout-ms N]\n\nRecursively find *.test.walz under tests/ by default. Explicit files may have any name.\nEach test runs in a fresh Wasmtime (core) or Node.js 24+ (core-js) process.\n--list checks and lists tests without executing them. -- ends option parsing.";
const COMMANDS: &[&str] = &[
    "print",
    "new",
    "build",
    "test",
    "bench",
    "bind",
    "type-at",
    "esm-metadata",
    "lsp",
    "check",
    "fmt",
    "fmt-check",
    "types",
    "wat",
    "expand",
    "macro-expand",
    "wit",
    "analyze",
    "ir",
    "contracts",
    "race-contracts",
    "verify",
    "proof",
    "run",
    "run-verified",
    "component",
    "component-wat",
    "core-js",
    "profile-text",
];

const CHECK_USAGE: &str = "Usage: walz check <file.walz> [--json] [--warn-performance] [--bindings policy.json]\n\nCheck types, effects, ownership, race contracts, and laws without running host code.\n--warn-performance adds advisory loop-copy and GC host-escape warnings for typed Walz.\nWarnings do not fail the check; --json keeps errors in diagnostics and advice in performance.\nNative WAT remains accepted; opaque expression coverage is reported explicitly.";

fn help(command: Option<&str>) -> ExitCode {
    println!(
        "{}",
        match command {
            Some("new") => cli_new::USAGE,
            Some("print") => cli_print::USAGE,
            Some("build") => cli_build::USAGE,
            Some("test") => TEST_USAGE,
            Some("bench") => cli_bench::USAGE,
            Some("run") => cli_run::USAGE,
            Some("check") => CHECK_USAGE,
            Some("esm-metadata") => cli_esm::USAGE,
            None | Some("--help" | "-h" | "help") => USAGE,
            Some(command) if COMMANDS.contains(&command) => USAGE,
            Some(command) => {
                eprintln!("unknown command {command:?}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    );
    ExitCode::SUCCESS
}

fn test_arguments(args: &[OsString]) -> Result<(Vec<PathBuf>, walz::runner::RunOptions), String> {
    let mut paths = Vec::new();
    let mut options = walz::runner::RunOptions::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--legacy-syntax") => options.legacy_syntax = true,
            Some("--list") => options.list = true,
            Some(flag @ ("--target" | "--bindings" | "--host")) => {
                let value = args
                    .next()
                    .filter(|v| !v.to_string_lossy().starts_with('-'))
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match flag {
                    "--target" => options
                        .execution
                        .set_target(value.to_str().ok_or("target must be UTF-8")?)?,
                    "--bindings" => options.execution.bindings = Some(value.into()),
                    "--host" => options.execution.host = Some(value.into()),
                    _ => unreachable!(),
                }
            }
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
    options.execution.validate()?;
    Ok((paths, options))
}

fn test_command(args: &[OsString], legacy: bool) -> ExitCode {
    use walz::runner::TestOutcome;
    let (paths, mut options) = match test_arguments(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    options.legacy_syntax |= legacy;
    let report = walz::runner::run(&paths, &options);
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
    policy: &walz::bindings::BindingPolicy,
    legacy: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(command, "fmt" | "fmt-check") {
        let original = std::fs::read_to_string(path)?;
        let formatted = if walz::review::is_source_path(path) {
            let translated = walz::review::from_watz(&original)
                .map_err(|e| format!("{}:{e}", path.display()))?;
            walz::review::to_watz(&translated.text)
        } else {
            walz::format::source(&original)
        }
        .map_err(|e| format!("{}:{e}", path.display()))?;
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
        walz::source::load_legacy(path)?
    } else {
        walz::source::load(path)?
    };
    if command == "macro-expand" {
        let report =
            walz::macros::report(loaded.text(), policy).map_err(|e| loaded.diagnostic(e))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report.located(&loaded))?
        );
        return Ok(true);
    }
    if matches!(command, "build" | "component")
        && let Some(output) = output
    {
        let mut outputs = vec![output.to_path_buf()];
        if command == "component" {
            let mut metadata = output.as_os_str().to_os_string();
            metadata.push(".json");
            outputs.push(metadata.into());
        }
        for output in &outputs {
            match std::fs::metadata(output) {
                Ok(_) => {
                    for input in loaded.paths().collect::<std::collections::BTreeSet<_>>() {
                        if same_file::is_same_file(input, output)? {
                            return Err(
                                "output must differ from source path and linked source files"
                                    .into(),
                            );
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        if outputs.len() == 2
            && outputs[0].exists()
            && outputs[1].exists()
            && same_file::is_same_file(&outputs[0], &outputs[1])?
        {
            return Err("component and metadata outputs must differ".into());
        }
    }
    execute_source(command, path, output, loaded.text(), policy).map_err(|error| match error
        .downcast::<walz::Error>(
    ) {
        Ok(error) => loaded.diagnostic(*error).into(),
        Err(error) => error,
    })
}

fn execute_source(
    command: &str,
    path: &Path,
    output: Option<&Path>,
    source: &str,
    policy: &walz::bindings::BindingPolicy,
) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(command, "core-js" | "profile-text") {
        let built = walz::component::core_js::compile_with_policy(source, policy)?;
        let profile = if command == "profile-text" {
            Some(walz::component::profiling::compile(source)?)
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
        let built = walz::component::compile_with_policy(source, policy)?;
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
            if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(output, built.wasm)?;
            std::fs::write(metadata, serde_json::to_vec_pretty(&built.manifest)?)?;
        }
        return Ok(true);
    }
    if command == "proof" {
        print!(
            "{}",
            walz::proof::emit(&walz::check_with_policy(source, policy)?)?
        );
        return Ok(true);
    }
    if command == "race-contracts" {
        let module = walz::check_with_policy(source, policy)?;
        let certificate =
            walz::race::certificate(&module)?.ok_or("race-contracts requires (@race-free)")?;
        println!("{}", serde_json::to_string_pretty(&certificate)?);
        return Ok(true);
    }
    if command == "contracts" {
        let module = walz::check_with_policy(source, policy)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&walz::ownership::validate(source, &module)?)?
        );
        return Ok(true);
    }
    if command == "wit" {
        if walz::composition::is_component(source) {
            let built = walz::composition::compile(source, policy)?;
            if let Some(e) = built.interface.unsupported {
                return Err(e.into());
            }
            print!("{}", built.wit);
            return Ok(true);
        }
        let module = walz::check_with_policy(source, policy)?;
        print!("{}", walz::wit::emit(&module)?);
        return Ok(true);
    }
    if command == "check" {
        if walz::composition::is_component(source) {
            walz::composition::compile(source, policy)?;
        } else if policy.imports.is_empty() {
            walz::validate(source)?;
        } else {
            walz::check_with_policy(source, policy)?;
        }
        println!("ok");
        return Ok(true);
    }
    // Source inspection depends only on the front end, not on Wasm emission.
    if matches!(command, "check" | "types" | "ir" | "analyze") {
        if walz::composition::is_component(source) {
            let built = walz::composition::compile(source, policy)?;
            let view = serde_json::json!({"mode":"component","interface":built.interface,"memoryGraph":built.manifest["memoryGraph"],"wat":built.wat});
            println!("{}", serde_json::to_string_pretty(&view)?);
            return Ok(true);
        }
        let module = match walz::check_with_policy(source, policy) {
            Ok(module) => module,
            Err(_) => {
                let compilation = walz::compile_with_policy(source, policy)?;
                match command {
                    "types" => println!(
                        "{}",
                        serde_json::to_string_pretty(&walz::native::types(&compilation.wasm)?)?
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
                serde_json::to_string_pretty(&walz::typing::report(&module))?
            ),
            "ir" => println!("{}", serde_json::to_string_pretty(&module)?),
            "analyze" => println!(
                "{}",
                serde_json::to_string_pretty(&walz::analysis::analyze(&module))?
            ),
            _ => unreachable!(),
        }
        return Ok(true);
    }
    let compiled = if matches!(command, "verify" | "run-verified") {
        walz::verified::compile(source)?
    } else {
        walz::compile_with_policy(source, policy)?
    };
    match command {
        "wat" | "expand" => print!("{}", compiled.wat),
        "build" => {
            let output = output.expect("argument validation requires an output path");
            if path == output {
                return Err("output must differ from source path".into());
            }
            if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(output, compiled.wasm)?;
        }
        "verify" => {
            if compiled.ir.as_ref().is_some_and(|m| m.race_free) {
                println!(
                    "ownership and conditional atomic-cell contracts verified; Wasm validated"
                );
            } else {
                println!("ownership contracts and laws verified; Wasm validated");
            }
        }
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
    if args.len() == 1 && matches!(args[0].to_str(), Some("--version" | "-V")) {
        println!("walz {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if (args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h" | "help")))
        || (args.len() == 2
            && (args[0] == "help" || matches!(args[1].to_str(), Some("--help" | "-h"))))
    {
        return help(
            if args[0] == "help" {
                args.get(1)
            } else if args.len() == 2 {
                args.first()
            } else {
                None
            }
            .and_then(|s| s.to_str()),
        );
    }
    if args.first().is_some_and(|s| s == "new") {
        return cli_new::command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "print") {
        return cli_print::command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "esm-metadata") {
        return cli_esm::command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "lsp") {
        let policy = if args.len() == 1 {
            Ok(Default::default())
        } else if args.len() == 3 && args[1] == "--bindings" {
            std::fs::read_to_string(&args[2])
                .map_err(|e| e.to_string())
                .and_then(|s| walz::bindings::BindingPolicy::parse(&s).map_err(|e| e.to_string()))
        } else {
            Err("Usage: walz lsp [--bindings policy.json]".into())
        };
        return match policy.and_then(|p| walz::lsp::serve_stdio(p).map_err(|e| e.to_string())) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        };
    }
    if args.first().is_some_and(|s| s == "bench") {
        return cli_bench::command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "test") {
        return test_command(&args[1..], false);
    }
    if args.first().is_some_and(|s| s == "build") {
        return cli_build::command(&args[1..], false);
    }
    if args.first().is_some_and(|s| s == "run") {
        return cli_run::command(&args[1..]);
    }
    let json = args
        .iter()
        .position(|s| s == "--json")
        .map(|i| args.remove(i))
        .is_some();
    let performance = args
        .iter()
        .position(|s| s == "--warn-performance")
        .map(|i| args.remove(i))
        .is_some();
    if performance && !args.first().is_some_and(|s| s == "check") {
        eprintln!("--warn-performance is supported by check\n{USAGE}");
        return ExitCode::from(2);
    }
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
        let world = if let Some(i) = args.iter().position(|s| s == "--world") {
            let Some(value) = args.get(i + 1).and_then(|s| s.to_str()).map(str::to_owned) else {
                eprintln!("--world requires a world name");
                return ExitCode::from(2);
            };
            args.drain(i..i + 2);
            Some(value)
        } else {
            None
        };
        if args.len() != 4 {
            eprintln!(
                "Usage: walz bind <input.wit> <policy.json> <output-directory> [--world name]"
            );
            return ExitCode::from(2);
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let generated = walz::wit_bindgen::generate(
                Path::new(&args[1]),
                world.as_deref(),
                &std::fs::read_to_string(&args[2])?,
            )?;
            let out = Path::new(&args[3]);
            if [
                "bindings.walz",
                "bindings.json",
                "types.walz",
                "exports.walz",
            ]
            .iter()
            .any(|n| out.join(n).exists())
            {
                return Err("binding outputs already exist".into());
            }
            std::fs::create_dir_all(out)?;
            std::fs::write(out.join("bindings.walz"), generated.linked_source)?;
            std::fs::write(out.join("bindings.json"), generated.policy)?;
            std::fs::write(out.join("types.walz"), generated.types_source)?;
            std::fs::write(out.join("exports.walz"), generated.export_source)?;
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
    let mut policy = walz::bindings::BindingPolicy::default();
    if let Some(i) = args.iter().position(|s| s == "--bindings") {
        let Some(path) = args.get(i + 1) else {
            eprintln!("--bindings requires a policy file");
            return ExitCode::from(2);
        };
        match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| walz::bindings::BindingPolicy::parse(&s).map_err(|e| e.to_string()))
        {
            Ok(p) => policy = p,
            Err(e) => {
                if json || args.first().is_some_and(|s| s == "type-at") {
                    return cli_inspection::failure(
                        if json {
                            "walz-check-v1"
                        } else {
                            "walz-type-at-v1"
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
    if command == "macro-expand" && args.iter().any(|a| a == "--text") {
        let paths = args
            .iter()
            .skip(1)
            .filter(|a| *a != "--text")
            .collect::<Vec<_>>();
        if paths.len() != 1 || args.len() != 3 {
            eprintln!("Usage: walz macro-expand [--text] <file.walz>");
            return ExitCode::from(2);
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let loaded = if legacy {
                walz::source::load_legacy(Path::new(paths[0]))?
            } else {
                walz::source::load(Path::new(paths[0]))?
            };
            let report =
                walz::macros::report(loaded.text(), &policy).map_err(|e| loaded.diagnostic(e))?;
            print!("{}", report.render(&loaded));
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
    if command == "type-at" {
        return cli_inspection::type_at(&args[1..], &policy);
    }
    if !COMMANDS.contains(&command)
        || args.len()
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
    if json || performance {
        return cli_inspection::check(path, &policy, legacy, json, performance);
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
