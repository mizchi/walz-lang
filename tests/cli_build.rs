use std::path::Path;
use std::process::{Command, Output};

fn cli(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn valid_wasm(path: &Path) {
    let bytes = std::fs::read(path).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .unwrap();
}

const SOURCE: &str = r#"(module
  (%use "./math.wlt" (func $answer))
  (func $main (export "main") (result i32) (call $answer)))"#;
const LIBRARY: &str = r#"(module
  (%export (func $answer))
  (func $answer (result i32) (+ 40 2)))"#;

#[test]
fn build_finds_a_project_entry_and_creates_its_output_directory() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("example app");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("src/main.wlt"), SOURCE).unwrap();
    std::fs::write(project.join("src/math.wlt"), LIBRARY).unwrap();
    success(cli(&project, &["build"]));
    valid_wasm(&project.join("target/main.wasm"));
    success(cli(
        directory.path(),
        &["build", "example app", "-o", "dist/app.wasm"],
    ));
    valid_wasm(&directory.path().join("dist/app.wasm"));
    // main.wlt at the root takes priority over src/main.wlt.
    std::fs::write(project.join("main.wlt"), "(module)").unwrap();
    success(cli(&project, &["build"]));
    assert_eq!(
        std::fs::read(project.join("target/main.wasm")).unwrap(),
        wat::parse_str("(module)").unwrap()
    );
}

#[test]
fn build_supports_default_named_and_legacy_positional_outputs() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("program.wlt"), "(module)").unwrap();
    for args in [
        vec!["build", "program.wlt"],
        vec!["build", "program.wlt", "-o", "out/short.wasm"],
        vec!["build", "--output", "out/long.wasm", "program.wlt"],
        vec!["build", "program.wlt", "out/positional.wasm"],
    ] {
        success(cli(directory.path(), &args));
    }
    for path in [
        "target/program.wasm",
        "out/short.wasm",
        "out/long.wasm",
        "out/positional.wasm",
    ] {
        valid_wasm(&directory.path().join(path));
    }
    std::fs::write(directory.path().join("-input.wlt"), "(module)").unwrap();
    success(cli(
        directory.path(),
        &["build", "-o", "out/dash.wasm", "--", "-input.wlt"],
    ));
    valid_wasm(&directory.path().join("out/dash.wasm"));
    std::fs::write(directory.path().join("my.program.wlt"), "(module)").unwrap();
    success(cli(directory.path(), &["build", "my.program.wlt"]));
    valid_wasm(&directory.path().join("target/my.program.wasm"));
}

#[test]
fn build_rejects_invalid_options_and_preserves_sources_on_output_aliases() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("program.wlt");
    std::fs::write(&source, "(module)").unwrap();
    for args in [
        vec!["build", "--unknown"],
        vec!["build", "-o"],
        vec!["build", "-o", "a", "--output", "b"],
        vec!["build", "program.wlt", "a.wasm", "-o", "b.wasm"],
        vec!["build", "a", "b", "c"],
    ] {
        assert_eq!(
            cli(directory.path(), &args).status.code(),
            Some(2),
            "{args:?}"
        );
    }
    for output in ["program.wlt", "./program.wlt"] {
        let result = cli(directory.path(), &["build", "program.wlt", "-o", output]);
        assert_eq!(result.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&result.stderr).contains("output must differ"));
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "(module)");
    }
    std::fs::hard_link(&source, directory.path().join("alias.wlt")).unwrap();
    assert_eq!(
        cli(directory.path(), &["build", "program.wlt", "alias.wlt"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "(module)");
    std::fs::write(&source, SOURCE).unwrap();
    std::fs::write(directory.path().join("math.wlt"), LIBRARY).unwrap();
    let result = cli(
        directory.path(),
        &["build", "program.wlt", "-o", "math.wlt"],
    );
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(directory.path().join("math.wlt")).unwrap(),
        LIBRARY
    );
}

#[test]
fn failed_builds_leave_no_output_and_missing_entries_explain_the_convention() {
    let directory = tempfile::tempdir().unwrap();
    let missing = cli(directory.path(), &["build"]);
    assert_eq!(missing.status.code(), Some(1));
    let error = String::from_utf8_lossy(&missing.stderr);
    assert!(
        error.contains("main.wlt") && error.contains("src/main.wlt"),
        "{error}"
    );
    std::fs::write(
        directory.path().join("main.wlt"),
        "(module (func $f (result i32) (f64.const 1)))",
    )
    .unwrap();
    let invalid = cli(directory.path(), &["build"]);
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("type mismatch"));
    assert!(!directory.path().join("target").exists());
}

#[test]
fn installed_command_help_and_version_require_no_runtime_or_source() {
    let directory = tempfile::tempdir().unwrap();
    for args in [vec!["help", "unknown"], vec!["unknown", "--help"]] {
        assert_eq!(cli(directory.path(), &args).status.code(), Some(2));
    }
    for args in [
        vec!["--help"],
        vec!["help"],
        vec!["build", "--help"],
        vec!["test", "--help"],
        vec!["bench", "--help"],
        vec!["help", "build"],
        vec!["--version"],
        vec!["-V"],
    ] {
        let output = success(
            Command::new(env!("CARGO_BIN_EXE_wlt"))
                .current_dir(directory.path())
                .env("PATH", "")
                .args(&args)
                .output()
                .unwrap(),
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.is_empty(), "{args:?}");
        if args[0] == "--version" || args[0] == "-V" {
            assert_eq!(text.trim(), concat!("wlt ", env!("CARGO_PKG_VERSION")));
        } else {
            assert!(text.contains("Usage:"), "{text}");
        }
    }
    std::fs::write(directory.path().join("main.wlt"), r#"(module (%use "wlt:core" (type $option)) (func $f (result (%inst $option i32)) (%option.some 42)))"#).unwrap();
    success(
        Command::new(env!("CARGO_BIN_EXE_wlt"))
            .current_dir(directory.path())
            .env("PATH", "")
            .arg("build")
            .output()
            .unwrap(),
    );
    valid_wasm(&directory.path().join("target/main.wasm"));
}

#[test]
fn discovery_skips_generated_directories_and_allows_explicit_opt_in() {
    let directory = tempfile::tempdir().unwrap();
    let test = r#"(module (%test "visible test" 1))"#;
    let bench = r#"(module (%bench "visible bench" (nop)))"#;
    std::fs::write(directory.path().join("visible.test.wlt"), test).unwrap();
    std::fs::write(directory.path().join("visible_bench.wlt"), bench).unwrap();
    for name in ["target", "node_modules", ".git"] {
        let path = directory.path().join(name);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("generated.test.wlt"), test).unwrap();
        std::fs::write(path.join("generated_bench.wlt"), bench).unwrap();
    }
    for (command, suffix, count) in [
        ("test", "generated.test.wlt", "1 tests;"),
        ("bench", "generated_bench.wlt", "1 benchmarks;"),
    ] {
        let listed = success(cli(directory.path(), &[command, ".", "--list"]));
        assert!(String::from_utf8_lossy(&listed.stdout).contains(count));
        success(cli(
            directory.path(),
            &[command, &format!("target/{suffix}"), "--list"],
        ));
        success(cli(directory.path(), &[command, "target", "--list"]));
    }
}

#[test]
fn runner_option_terminators_preserve_filenames_that_look_like_global_flags() {
    let directory = tempfile::tempdir().unwrap();
    for (command, source) in [
        ("test", r#"(module (%test "--legacy-syntax" 1))"#),
        ("bench", r#"(module (%bench "--legacy-syntax" (nop)))"#),
    ] {
        for name in ["--json", "--bindings", "--legacy-syntax"] {
            std::fs::write(directory.path().join(name), source).unwrap();
            success(cli(directory.path(), &[command, "--list", "--", name]));
        }
        success(cli(
            directory.path(),
            &[
                command,
                "--list",
                "--filter",
                "--legacy-syntax",
                "--",
                "--json",
            ],
        ));
    }
}
