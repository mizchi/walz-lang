use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

pub const USAGE: &str = "Usage: walz run [file.walz|directory] [--invoke export] [--target core|core-js]\n               [--bindings policy.json] [--host host.mjs] [--timeout-ms N] [--exit-code]\n               [--stdio [--input file] [--output file]]\n               [--read-file NAME=PATH] [--write-file NAME=PATH] [-- arguments...]\n\nDefault entry: main.walz, then src/main.walz. Default target: core (Wasmtime).\ncore-js uses Node.js 24+ and supports async imports. --stdio explicitly grants text I/O.\nArguments are also available to core-js guests through walz:host/cli@0.1.0.arguments.";

#[derive(Default)]
struct Options {
    source: Option<PathBuf>,
    target: String,
    timeout_ms: Option<u32>,
    invoke: String,
    bindings: Option<PathBuf>,
    host: Option<PathBuf>,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    stdio: bool,
    exit_code: bool,
    legacy: bool,
    args: Vec<OsString>,
    read_files: std::collections::BTreeMap<String, PathBuf>,
    write_files: std::collections::BTreeMap<String, PathBuf>,
}
fn arguments(args: &[OsString]) -> Result<Options, String> {
    let mut options = Options {
        target: "core".into(),
        invoke: "main".into(),
        ..Options::default()
    };
    let mut args = args.iter();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some(flag @ ("--read-file" | "--write-file")) => {
                let value = args
                    .next()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| format!("{flag} requires name=path"))?;
                let (name, path) = value
                    .split_once('=')
                    .filter(|(n, p)| !n.is_empty() && !p.is_empty())
                    .ok_or_else(|| format!("{flag} requires name=path"))?;
                let files = if flag == "--read-file" {
                    &mut options.read_files
                } else {
                    &mut options.write_files
                };
                if files.insert(name.into(), path.into()).is_some() {
                    return Err(format!("duplicate file grant {name}"));
                }
            }
            Some("--") => {
                options.args.extend(args.cloned());
                break;
            }
            Some("--timeout-ms") => {
                if !seen.insert("--timeout-ms") {
                    return Err("duplicate option --timeout-ms".into());
                }
                options.timeout_ms = Some(
                    args.next()
                        .and_then(|v| v.to_str())
                        .and_then(|v| v.parse::<u32>().ok())
                        .filter(|n| *n <= 2147483647)
                        .ok_or("--timeout-ms requires an integer between 0 and 2147483647")?,
                );
            }
            Some("--stdio") => options.stdio = true,
            Some("--exit-code") => options.exit_code = true,
            Some("--legacy-syntax") => options.legacy = true,
            Some(
                flag @ ("--target" | "--invoke" | "--bindings" | "--host" | "--input" | "--output"),
            ) => {
                if !seen.insert(flag) {
                    return Err(format!("duplicate option {flag}"));
                }
                let value = args
                    .next()
                    .filter(|v| !v.to_string_lossy().starts_with('-'))
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match flag {
                    "--target" => {
                        options.target = value.to_str().ok_or("target must be UTF-8")?.into()
                    }
                    "--invoke" => {
                        options.invoke = value.to_str().ok_or("export name must be UTF-8")?.into()
                    }
                    "--bindings" => options.bindings = Some(value.into()),
                    "--host" => options.host = Some(value.into()),
                    "--input" => options.input = Some(value.into()),
                    "--output" => options.output = Some(value.into()),
                    _ => unreachable!(),
                }
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown run option {flag}"));
            }
            _ if options.source.is_none() => options.source = Some(arg.into()),
            _ => return Err("use -- before program arguments".into()),
        }
    }
    if !matches!(options.target.as_str(), "core" | "core-js") {
        return Err("target must be core or core-js".into());
    }
    if (options.timeout_ms.is_some()
        || options.stdio
        || options.host.is_some()
        || !options.read_files.is_empty()
        || !options.write_files.is_empty())
        && options.target != "core-js"
    {
        return Err("host capabilities require --target core-js".into());
    }
    if !options.stdio && (options.input.is_some() || options.output.is_some()) {
        return Err("--input and --output require --stdio".into());
    }
    Ok(options)
}

pub fn command(args: &[OsString]) -> ExitCode {
    let options = match arguments(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(options) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
fn execute(options: Options) -> Result<u8, Box<dyn std::error::Error>> {
    let path = options.source.unwrap_or_else(|| PathBuf::from("."));
    let path = if path.is_dir() {
        super::cli_build::entry(&path)?
    } else {
        path
    };
    let loaded = if options.legacy {
        walz::source::load_legacy(&path)?
    } else {
        walz::source::load(&path)?
    };
    let policy = options
        .bindings
        .as_ref()
        .map(std::fs::read_to_string)
        .transpose()?
        .map(|s| walz::bindings::BindingPolicy::parse(&s))
        .transpose()?
        .unwrap_or_default();
    let dir = tempfile::tempdir()?;
    if options.target == "core-js" {
        for output in options
            .write_files
            .values()
            .chain(options.output.iter())
            .filter(|p| p.exists())
        {
            for input in loaded
                .paths()
                .chain(options.input.as_deref())
                .chain(options.host.as_deref())
                .chain(options.bindings.as_deref())
                .chain(options.read_files.values().map(|p| p.as_path()))
                .filter(|p| p.exists())
            {
                if same_file::is_same_file(input, output)? {
                    return Err("input and output must differ".into());
                }
            }
        }
        if let Some(output) = &options.output
            && output.exists()
        {
            for source in loaded.paths().chain(options.input.as_deref()) {
                if same_file::is_same_file(source, output)? {
                    return Err("input and output must differ".into());
                }
            }
        }
        let built = walz::component::core_js::compile_with_policy(loaded.text(), &policy)
            .map_err(|e| loaded.diagnostic(e))?;
        let export = built
            .exports
            .get(&options.invoke)
            .ok_or_else(|| format!("unknown export {}", options.invoke))?;
        let absolute = |p: PathBuf| std::env::current_dir().map(|d| d.join(p));
        let cwd = std::env::current_dir()?;
        let read_files = options
            .read_files
            .iter()
            .map(|(n, p)| (n, cwd.join(p)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let write_files = options
            .write_files
            .iter()
            .map(|(n, p)| (n, cwd.join(p)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let config = serde_json::json!({"entry":export.js_name,"timeoutMs":options.timeout_ms,"arity":export.arity,"args":options.args.iter().map(|v|v.to_str().ok_or("core-js arguments must be UTF-8")).collect::<Result<Vec<_>,_>>()?,"stdio":options.stdio,"exitCode":options.exit_code,"input":options.input.map(absolute).transpose()?,"output":options.output.map(absolute).transpose()?,"host":options.host.map(std::fs::canonicalize).transpose()?,"readFiles":read_files,"writeFiles":write_files});
        super::cli_js::write(dir.path(), &built)?;
        for (name, source) in [
            ("cli-run.mjs", include_str!("../runtime/cli-run.mjs")),
            ("cli-host.mjs", include_str!("../runtime/cli-host.mjs")),
            ("node-io.mjs", include_str!("../runtime/node-io.mjs")),
            ("text-io.mjs", include_str!("../runtime/text-io.mjs")),
            (
                "wasi-filesystem.mjs",
                include_str!("../runtime/wasi-filesystem.mjs"),
            ),
            (
                "node-filesystem.mjs",
                include_str!("../runtime/node-filesystem.mjs"),
            ),
        ] {
            std::fs::write(dir.path().join(name), source)?;
        }
        let config_file = dir.path().join("run.json");
        std::fs::write(&config_file, serde_json::to_vec(&config)?)?;
        let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let requested = interrupted.clone();
        ctrlc::set_handler(move || requested.store(true, std::sync::atomic::Ordering::Relaxed))?;
        let mut child = Command::new("node")
            .arg(dir.path().join("cli-run.mjs"))
            .arg(config_file)
            .spawn()
            .map_err(|e| format!("could not start Node.js 24+: {e}"))?;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if interrupted.swap(false, std::sync::atomic::Ordering::Relaxed) {
                #[cfg(unix)]
                if let Some(pid) = rustix::process::Pid::from_raw(child.id().try_into()?) {
                    // Keep the parent alive until JS has unwound the guest and
                    // finished host disposal. A terminal may signal both ends.
                    let _ = rustix::process::kill_process(pid, rustix::process::Signal::INT);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        return Ok(status
            .code()
            .and_then(|n| u8::try_from(n).ok())
            .unwrap_or(1));
    }
    let compiled =
        walz::compile_with_policy(loaded.text(), &policy).map_err(|e| loaded.diagnostic(e))?;
    let file = dir.path().join("program.wasm");
    std::fs::write(&file, compiled.wasm)?;
    let mut command = Command::new("wasmtime");
    command
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            &options.invoke,
        ])
        .arg(file)
        .args(options.args)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit());
    if options.exit_code {
        let output = command
            .output()
            .map_err(|e| format!("could not start wasmtime: {e}"))?;
        if !output.status.success() {
            return Ok(1);
        }
        return String::from_utf8(output.stdout)?
            .trim()
            .parse::<u8>()
            .map_err(|_| "--exit-code requires an integer result between 0 and 255".into());
    }
    Ok(
        if command
            .status()
            .map_err(|e| format!("could not start wasmtime: {e}"))?
            .success()
        {
            0
        } else {
            1
        },
    )
}
