//! Build argument policy, separate from source loading and compilation.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const USAGE: &str = "Usage: walz build [file.walz|directory] [-o output] [--target core|core-js|component]\n                [--bindings policy.json] [--wit] [--wit-input world.wit [--world name]]\n       walz build <file.walz> <output.wasm>\n\nDefaults: main.walz, src/main.walz, main.watz, src/main.watz. Output is target/<name>.wasm (core),\ntarget/<name>-js/ (core-js), or target/<name>.component.wasm (component).\nDirectories use their own target/ directory. -- ends option parsing.";

#[derive(Default)]
struct Arguments {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    bindings: Option<PathBuf>,
    legacy: bool,
    target: Option<String>,
    wit: Option<PathBuf>,
    emit_wit: bool,
    world: Option<String>,
}

fn arguments(args: &[OsString]) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    let mut positional = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--target") => {
                if parsed.target.is_some() {
                    return Err("duplicate option --target".into());
                }
                let value = args
                    .next()
                    .and_then(|s| s.to_str())
                    .ok_or("--target requires core, core-js, or component")?;
                if !matches!(value, "core" | "core-js" | "component") {
                    return Err("--target requires core, core-js, or component".into());
                }
                parsed.target = Some(value.into());
            }
            Some("--world") => {
                if parsed.world.is_some() {
                    return Err("duplicate option --world".into());
                }
                parsed.world = Some(
                    args.next()
                        .and_then(|v| v.to_str())
                        .filter(|v| !v.starts_with('-'))
                        .ok_or("--world requires a name")?
                        .into(),
                );
            }
            Some("--wit" | "--emit-wit") => {
                if parsed.emit_wit {
                    return Err("duplicate option --wit".into());
                }
                parsed.emit_wit = true;
            }
            Some("--legacy-syntax") => parsed.legacy = true,
            Some("-o" | "--output" | "--bindings" | "--wit-input") => {
                let slot = if arg == "--wit-input" {
                    &mut parsed.wit
                } else if arg == "--bindings" {
                    &mut parsed.bindings
                } else {
                    &mut parsed.output
                };
                if slot.is_some() {
                    return Err(format!("duplicate option {}", arg.to_string_lossy()));
                }
                let value = args
                    .next()
                    .filter(|s| !s.to_string_lossy().starts_with('-'))
                    .ok_or_else(|| format!("{} requires a path", arg.to_string_lossy()))?;
                *slot = Some(PathBuf::from(value));
            }
            Some("--") => {
                positional.extend(args.map(PathBuf::from));
                break;
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown build option {flag:?}"));
            }
            _ => positional.push(PathBuf::from(arg)),
        }
    }
    if positional.len() > 2 || (positional.len() == 2 && parsed.output.is_some()) {
        return Err("expected one input and one output; use -o or a positional output".into());
    }
    let mut positional = positional.into_iter();
    parsed.input = positional.next();
    parsed.output = parsed.output.or_else(|| positional.next());
    if parsed.world.is_some() && parsed.wit.is_none() {
        return Err("--world requires --wit-input".into());
    }

    Ok(parsed)
}

pub(super) fn entry(directory: &Path) -> Result<PathBuf, String> {
    [
        directory.join("main.walz"),
        directory.join("src/main.walz"),
        directory.join("main.watz"),
        directory.join("src/main.watz"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| {
        format!(
            "{}: no entry file; expected main.walz or src/main.walz (also .watz), or pass a file to walz build",
            directory.display()
        )
    })
}

pub fn command(args: &[OsString], legacy: bool) -> ExitCode {
    let parsed = match arguments(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let input = parsed.input.unwrap_or_else(|| PathBuf::from("."));

        let (source, root) = if input.is_dir() {
            (entry(&input)?, input)
        } else {
            (input, PathBuf::from("."))
        };
        let loaded = if legacy || parsed.legacy {
            walz::source::load_legacy(&source)?
        } else {
            walz::source::load(&source)?
        };
        let is_component = walz::composition::is_component(loaded.text());
        let target =
            parsed
                .target
                .as_deref()
                .unwrap_or(if is_component { "component" } else { "core" });
        if is_component && target != "component" {
            return Err("a component entry requires the component target".into());
        }
        if parsed.wit.is_some() && target != "component" {
            return Err("--wit-input requires a component target".into());
        }
        if parsed.emit_wit && target != "component" {
            return Err("--wit requires a component target".into());
        }
        let output = if let Some(output) = parsed.output {
            output
        } else {
            let mut name = source
                .file_stem()
                .ok_or("expected a source file or project directory")?
                .to_os_string();
            name.push(match target {
                "core-js" => "-js",
                "component" => ".component.wasm",
                _ => ".wasm",
            });
            root.join("target").join(name)
        };
        let policy = if let Some(path) = &parsed.bindings {
            walz::bindings::BindingPolicy::parse(&std::fs::read_to_string(path)?)?
        } else {
            walz::bindings::BindingPolicy::default()
        };
        let mut destinations = vec![output.clone()];
        let mut metadata = output.as_os_str().to_os_string();
        metadata.push(".json");
        let metadata = PathBuf::from(metadata);
        if target == "component" {
            destinations.push(metadata.clone());
        }
        let wit_output = output.parent().unwrap_or(Path::new(".")).join("world.wit");
        if parsed.emit_wit {
            destinations.push(wit_output.clone());
        }
        for out in &destinations {
            for input in loaded
                .paths()
                .chain(parsed.wit.as_deref())
                .chain(parsed.bindings.as_deref())
                .filter(|p| p.exists())
            {
                if out.exists() && same_file::is_same_file(input, out)? {
                    return Err(
                        "output must differ from source, WIT, policy and linked files".into(),
                    );
                }
            }
        }
        for (i, a) in destinations.iter().enumerate() {
            for b in &destinations[..i] {
                if a == b || a.exists() && b.exists() && same_file::is_same_file(a, b)? {
                    return Err("build output paths must differ".into());
                }
            }
        }
        if target == "component" {
            let built = if let Some(wit) = &parsed.wit {
                walz::component::compile_with_world(
                    loaded.text(),
                    &policy,
                    wit,
                    parsed.world.as_deref(),
                )
            } else {
                walz::component::compile_with_policy(loaded.text(), &policy)
            }
            .map_err(|e| loaded.diagnostic(e))?;
            // Validate every requested artifact before publishing any of them.
            let wit = if parsed.emit_wit {
                Some(walz::composition::inspect(&built.wasm)?.interface.wit)
            } else {
                None
            };
            let manifest = serde_json::to_vec_pretty(&built.manifest)?;
            if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, built.wasm)?;
            std::fs::write(metadata, manifest)?;
            if let Some(wit) = wit {
                std::fs::write(&wit_output, wit)?;
            }
        } else if target == "core-js" {
            let mut built = walz::component::core_js::compile_with_policy(loaded.text(), &policy)
                .map_err(|e| loaded.diagnostic(e))?;
            let inputs = loaded
                .paths()
                .map(Path::to_path_buf)
                .chain(
                    parsed
                        .bindings
                        .as_ref()
                        .map(|path| path.canonicalize())
                        .transpose()?,
                )
                .collect::<std::collections::BTreeSet<_>>();
            built.manifest["sourceFiles"] = serde_json::to_value(&inputs)?;
            for name in super::cli_js::FILES {
                let path = output.join(name);
                if path.exists() {
                    for input in &inputs {
                        if same_file::is_same_file(input, &path)? {
                            return Err(
                                "output must differ from source path and linked source files"
                                    .into(),
                            );
                        }
                    }
                }
            }
            super::cli_js::write(&output, &built)?;
        } else {
            super::execute_source(
                if target == "component" {
                    "component"
                } else {
                    "build"
                },
                &source,
                Some(&output),
                loaded.text(),
                &policy,
            )
            .map_err(|e| match e.downcast::<walz::Error>() {
                Ok(e) => Box::<dyn std::error::Error>::from(loaded.diagnostic(*e)),
                Err(e) => e,
            })?;
        }
        println!("Built {}", output.display());
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
