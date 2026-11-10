use std::process::Command;

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_walz"));
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
    command
}

#[test]
fn runs_all_gc_examples() {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../examples/catalog.json")).unwrap();
    for example in catalog["examples"].as_array().unwrap() {
        let Some(expected) = example["expected"].as_str() else {
            continue;
        };
        let name = example["path"].as_str().unwrap();
        let output = command().args(["run", name]).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            expected,
            "{name}"
        );
        println!("{name} => {expected}");
    }
}

#[test]
fn expected_runtime_failures_are_not_counted_as_successful_examples() {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../examples/catalog.json")).unwrap();
    for example in catalog["examples"].as_array().unwrap() {
        let Some(expected_error) = example["runtime_error"].as_str() else {
            continue;
        };
        let path = example["path"].as_str().unwrap();
        let output = command().args(["run", path]).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected_error), "{path}: {error}");
    }
}

#[test]
fn emits_valid_wasm_and_inspectable_wat_and_ir() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("program.wasm");
    let output = command()
        .args(["build", "examples/closure.walz"])
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(destination).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let output = command()
        .args(["wat", "examples/closure.walz"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let wat = String::from_utf8(output.stdout).unwrap();
    assert!(wat.contains("struct.new $Adder"));
    assert!(!wat.contains("closure.new"));
    let output = command()
        .args(["ir", "examples/closure.walz"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let ir: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(ir["functions"].as_array().unwrap().len() >= 2);
}

#[test]
fn reports_effects_as_json_and_checks_without_executing() {
    let output = command()
        .args(["analyze", "examples/counter.walz"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let functions = report["functions"].as_array().unwrap();
    let increment = functions
        .iter()
        .find(|f| f["name"] == "$Increment")
        .unwrap();
    assert_eq!(increment["effects"]["writes"], true);
    assert_eq!(increment["captures"][0], "$cell");
    let output = command()
        .args(["check", "examples/closure.walz"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ok\n");
}

#[test]
fn reports_expression_and_binding_types_without_running_the_program() {
    let output = command()
        .args(["types", "examples/ast-interpreter.walz"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let eval = report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|function| function["name"] == "$eval")
        .unwrap();
    assert_eq!(eval["result"], "i64");
    let left = eval["locals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|local| local["name"] == "$left")
        .unwrap();
    assert_eq!(left["ty"], "(ref $Expr)");
    assert_eq!(left["kind"], "Match");
    assert!(!eval["expressions"].as_array().unwrap().is_empty());
}

#[test]
fn static_type_errors_use_source_type_names() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wrong-type.walz");
    std::fs::write(&path, "(module\n  (type $Left (struct))\n  (type $Right (struct))\n  (func $f (result (ref $Left)) (@let $wrong (struct.new $Right) (local.get $wrong))))").unwrap();
    for command_name in ["check", "types", "build"] {
        let mut cli = command();
        cli.arg(command_name).arg(&path);
        if command_name == "build" {
            cli.arg(directory.path().join("wrong.wasm"));
        }
        let output = cli.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("wrong-type.walz:4:"), "{error}");
        assert!(
            error.contains("type mismatch: expected (ref $Left), got (ref $Right)"),
            "{error}"
        );
    }
    assert!(!directory.path().join("wrong.wasm").exists());
}

#[test]
fn rejects_invalid_usage_and_reports_source_diagnostics() {
    let help = command().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));
    for args in [vec![], vec!["unknown"], vec!["build", "--unknown"]] {
        assert_eq!(
            command().args(args).output().unwrap().status.code(),
            Some(2)
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("bad.walz");
    std::fs::write(
        &source,
        "(module\n  (func $f (result i64) (local.get $hidden)))",
    )
    .unwrap();
    let output = command().arg("check").arg(source).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("bad.walz:2:"), "{error}");
    assert!(error.contains("unbound local"), "{error}");
}
