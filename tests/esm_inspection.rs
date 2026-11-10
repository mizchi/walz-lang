use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn inspect(path: &std::path::Path, source: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("esm-metadata")
        .arg(path)
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn metadata_uses_unsaved_content_and_original_utf8_export_positions_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.walz");
    let source = ";; 日本語 😀\n(module (func $add (export \"add-one\") (param $n i64) (result i64) (@effect pure) (+ $n 1)))";
    let report = inspect(&path, source);
    assert_eq!(report["schema"], "walz-esm-metadata-v1");
    assert_eq!(report["ok"], true, "{report}");
    let export = &report["esm"]["exports"]["add-one"];
    assert_eq!(export["jsName"], "addOne");
    assert_eq!(export["params"], json!(["i64"]));
    let span = &export["declaration"]["span"];
    assert_eq!(
        &source[span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize],
        "add-one"
    );
    assert!(!path.exists());
}

#[test]
fn metadata_loads_static_dependencies_and_preserves_checker_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.walz");
    let helper = dir.path().join("helper.walz");
    std::fs::write(
        &helper,
        "(module (@export (func $answer)) (func $answer (result i32) (@effect pure) 42))",
    )
    .unwrap();
    let source = "(module (@use \"./helper.walz\" (func $answer)) (func $main (export \"main\") (result i32) (@effect pure) (call $answer)))";
    let report = inspect(&path, source);
    assert_eq!(report["ok"], true, "{report}");
    assert!(
        report["sourceFiles"]
            .as_array()
            .unwrap()
            .contains(&json!(helper.canonicalize().unwrap()))
    );
    let error = inspect(
        &path,
        "(module (func $main (export \"main\") (result i32) (@effect pure) $missing))",
    );
    assert_eq!(error["ok"], false);
    assert!(
        error["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("missing")
    );
    assert_eq!(
        error["diagnostics"][0]["primary"]["location"]["path"],
        json!(dir.path().canonicalize().unwrap().join("main.walz"))
    );
}

#[test]
fn metadata_keeps_effect_ownership_and_portable_boundary_checks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.walz");
    for (source, expected) in [
        (
            "(module (func $main (export \"main\") (@let $x (@box.new i64 1) (block (@box.close (@move $x)) (@box.close (@move $x))))))",
            "already moved",
        ),
        (
            "(module (type $C (struct)) (func $main (export \"main\") (result (ref $C)) (@effect pure) (struct.new $C)))",
            "effect contract",
        ),
        (
            "(module (func $main (export \"main\") (param $v v128) (result v128) (@effect pure) $v))",
            "v128",
        ),
    ] {
        let report = inspect(&path, source);
        assert_eq!(report["ok"], false, "{report}");
        assert!(
            report["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{report}"
        );
    }
}

#[test]
fn unused_macro_templates_cannot_capture_export_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let source = r#"(module
      (func $main (export "main") (result i32) (@effect pure) 42)
      (@macro $unused (rule () (export "main"))))"#;
    let report = inspect(&dir.path().join("main.walz"), source);
    assert_eq!(report["ok"], true, "{report}");
    assert_eq!(
        report["esm"]["exports"]["main"]["declaration"]["span"]["start"],
        source.find("\"main\"").unwrap() + 1
    );
}
