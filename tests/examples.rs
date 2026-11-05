use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    examples: Vec<Example>,
    probes: Vec<Probe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Example {
    path: String,
    pattern: String,
    expected: Option<String>,
    runtime_error: Option<String>,
    #[serde(default)]
    checks: Vec<Invocation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    export: String,
    args: Vec<String>,
    expected: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    path: String,
    diagnostic: String,
    limitation: String,
    #[serde(default)]
    standard_wat: bool,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn catalog() -> Catalog {
    serde_json::from_str(&std::fs::read_to_string(root().join("examples/catalog.json")).unwrap())
        .unwrap()
}

fn invoke(path: &Path, export: &str, args: &[String]) -> Output {
    Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            export,
        ])
        .arg(path)
        .args(args)
        .output()
        .expect("example tests require wasmtime on PATH")
}

#[test]
fn every_catalog_example_is_checked_and_produces_valid_wasm() {
    let catalog = catalog();
    let mut paths = std::collections::BTreeSet::new();
    for example in &catalog.examples {
        assert!(
            paths.insert(&example.path),
            "duplicate example {}",
            example.path
        );
        assert!(
            example.expected.is_some() ^ example.runtime_error.is_some(),
            "{} needs one execution expectation",
            example.path
        );
        let source = std::fs::read_to_string(root().join(&example.path)).unwrap();
        let compiled = wlt::compile(&source)
            .unwrap_or_else(|error| panic!("{} ({}): {error}", example.path, example.pattern));
        wasmparser::Validator::new()
            .validate_all(&compiled.wasm)
            .unwrap();
        if compiled.ir.is_some() {
            assert!(!compiled.analysis.opaque);
            assert!(!compiled.analysis.functions.is_empty());
        } else {
            assert!(compiled.analysis.opaque);
            assert!(
                !wlt::native::types(&compiled.wasm)
                    .unwrap()
                    .functions
                    .is_empty()
            );
            let effects = serde_json::to_value(compiled.analysis.module_effects).unwrap();
            assert!(
                effects
                    .as_object()
                    .unwrap()
                    .values()
                    .all(|flag| flag == true)
            );
        }
        println!("checked {}: {}", example.path, example.pattern);
    }
    // Keep uncatalogued examples and probes from silently escaping checks.
    for probe in &catalog.probes {
        assert!(paths.insert(&probe.path), "duplicate probe {}", probe.path);
    }
    for directory in ["examples", "examples/probes"] {
        for entry in std::fs::read_dir(root().join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "wlt") {
                let relative = format!(
                    "{directory}/{}",
                    path.file_name().unwrap().to_str().unwrap()
                );
                assert!(
                    paths.contains(&relative),
                    "uncatalogued example: {relative}"
                );
            }
        }
    }
}

#[test]
fn additional_inputs_exercise_boundary_and_failure_results() {
    for example in catalog().examples {
        if example.checks.is_empty() {
            continue;
        }
        let source = std::fs::read_to_string(root().join(&example.path)).unwrap();
        let compiled = wlt::compile(&source).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("program.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        for check in example.checks {
            let output = invoke(&path, &check.export, &check.args);
            assert!(
                output.status.success(),
                "{} {}: {}",
                example.path,
                check.export,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                check.expected,
                "{} {} {:?}",
                example.path,
                check.export,
                check.args
            );
        }
    }
}

#[test]
fn capability_probes_record_reproducible_language_limits() {
    for probe in catalog().probes {
        let source = std::fs::read_to_string(root().join(&probe.path)).unwrap();
        if probe.standard_wat {
            let wasm = wat::parse_str(&source).unwrap();
            wasmparser::Validator::new().validate_all(&wasm).unwrap();
            // These three fixtures are complete standard-WAT programs returning 42.
            // A small stack also checks that the explicit tail-call case stays flat.
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("standard.wasm");
            std::fs::write(&path, wasm).unwrap();
            let output = Command::new("wasmtime")
                .args([
                    "run",
                    "-W",
                    "tail-call=y,max-wasm-stack=32768",
                    "--invoke",
                    "main",
                ])
                .arg(&path)
                .output()
                .expect("standard-Wasm probes require wasmtime on PATH");
            assert!(
                output.status.success(),
                "{}: {}",
                probe.path,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                "42",
                "{}",
                probe.path
            );
        }
        let error = match wlt::compile(&source) {
            Ok(_) => panic!(
                "{} is now supported; update the assessment and catalog",
                probe.path
            ),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(&probe.diagnostic), "{}: {error}", probe.path);
        println!("{}: {} ({error})", probe.path, probe.limitation);
    }
}

#[test]
fn representative_program_effects_remain_visible() {
    for (path, function, expected) in [
        ("hash-map", "$put", "writes"),
        ("ast-interpreter", "$eval", "may_diverge"),
        ("higher-order", "$map", "allocates"),
        ("graph-cycle", "$main", "writes"),
        ("result", "$divide", "may_trap"),
        ("numeric", "$sqrt", "may_diverge"),
        ("host-import", "$main", "host"),
    ] {
        let source = std::fs::read_to_string(root().join(format!("examples/{path}.wlt"))).unwrap();
        let compiled = wlt::compile(&source).unwrap();
        let function = compiled
            .analysis
            .functions
            .iter()
            .find(|entry| entry.name == function)
            .unwrap();
        let effects = serde_json::to_value(function.effects).unwrap();
        assert_eq!(
            effects[expected], true,
            "{path}: {} must expose {expected}",
            function.name
        );
    }
}
