use serde_json::Value;
use std::{path::Path, process::Command};

fn cli(args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    (out.status.code().unwrap(), json)
}

fn position(source: &str, needle: &str) -> (String, String) {
    let before = &source[..source.find(needle).unwrap()];
    (
        (before.bytes().filter(|b| *b == b'\n').count() + 1).to_string(),
        (before.rsplit('\n').next().unwrap().chars().count() + 1).to_string(),
    )
}

#[test]
fn json_check_keeps_success_failure_and_native_validation_machine_readable() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("source.wlt");
    let path = file.to_str().unwrap();
    for source in [
        "(module (func $f (nop)))",
        "(module (memory 1) (func (result i32) i32.const 42))",
    ] {
        std::fs::write(&file, source).unwrap();
        let (code, report) = cli(&["check", "--json", path]);
        assert_eq!(code, 0);
        assert_eq!(report["schema"], "wlt-check-v1");
        assert_eq!(report["ok"], true);
        assert_eq!(report["diagnostics"], serde_json::json!([]));
    }
    std::fs::write(&file, "(module (func $f (result i64) (i32.const 1)))").unwrap();
    let (code, report) = cli(&["check", path, "--json"]);
    assert_eq!(code, 1);
    assert_eq!(report["ok"], false);
    assert_eq!(report["diagnostics"][0]["code"], "E_TYPE_MISMATCH");
    assert_eq!(
        Path::new(
            report["diagnostics"][0]["primary"]["location"]["path"]
                .as_str()
                .unwrap()
        ),
        file.canonicalize().unwrap()
    );
    let (code, report) = cli(&[
        "check",
        "--json",
        dir.path().join("missing.wlt").to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_TOOL");
    let (code, report) = cli(&[
        "check",
        "--json",
        path,
        "--bindings",
        dir.path().join("missing.json").to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_TOOL");
    std::fs::write(&file, "(module (%law \"false\" (i32.const 0)))").unwrap();
    let (code, report) = cli(&["check", "--json", path]);
    assert_eq!(code, 1);
    assert_eq!(report["ok"], false);
}

#[test]
fn type_queries_map_imported_unicode_sources_and_preserve_aliases_and_instances() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("main.wlt");
    let child = dir.path().join("lib.wlt");
    let source = "(module\n (%export (func $id) (func $count))\n (%type.alias $Count i64)\n (func $id (%type.params $T) (param $x $T) (result $T) (%effect pure) (local.get $x))\n (func $count (result $Count) (%effect pure) ;; 日本語🦀\n  (; 型🦀 ;) (%let $n $Count (i64.const 42) (local.get $n))))";
    std::fs::write(&child, source).unwrap();
    std::fs::write(&root, "(module (%import \"lib.wlt\" (func $id) (func $count)) (func $a (result i32) (call (%inst $id i32) (i32.const 1))) (func $b (result i64) (call (%inst $id i64) (i64.const 2))))").unwrap();
    let (line, column) = position(source, "$n $Count");
    let (code, report) = cli(&[
        "type-at",
        root.to_str().unwrap(),
        &line,
        &column,
        "--source",
        child.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(report["mode"], "typed");
    assert_eq!(report["matches"][0]["ty"], "i64");
    assert_eq!(report["matches"][0]["source_type"], "$Count");
    assert_eq!(report["matches"][0]["function"], "$count");
    assert_eq!(
        Path::new(
            report["matches"][0]["location"]["location"]["path"]
                .as_str()
                .unwrap()
        ),
        child.canonicalize().unwrap()
    );
    let (line, column) = position(source, "$Count) (%effect");
    let (_, report) = cli(&[
        "type-at",
        root.to_str().unwrap(),
        &line,
        &column,
        "--source",
        child.to_str().unwrap(),
    ]);
    assert_eq!(report["matches"][0]["ty"], "i64");
    assert_eq!(report["matches"][0]["source_type"], "$Count");
    let (line, column) = position(source, "(local.get $x)");
    let (_, report) = cli(&[
        "type-at",
        root.to_str().unwrap(),
        &line,
        &column,
        "--source",
        child.to_str().unwrap(),
    ]);
    let mut types = report["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["ty"].as_str().unwrap())
        .collect::<Vec<_>>();
    types.sort();
    assert_eq!(types, ["i32", "i64"]);
    for item in report["matches"].as_array().unwrap() {
        assert_eq!(item["function"], "$id");
        assert_eq!(item["source_type"], "$T");
        assert!(!item["instantiations"].as_array().unwrap().is_empty());
    }
    let (line, column) = position(source, "日本語");
    let (_, report) = cli(&[
        "type-at",
        root.to_str().unwrap(),
        &line,
        &column,
        "--source",
        child.to_str().unwrap(),
    ]);
    assert!(report["matches"].as_array().unwrap().is_empty());
}

#[test]
fn type_queries_validate_coordinates_errors_and_native_limits() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file.wlt");
    let path = file.to_str().unwrap();
    std::fs::write(
        &file,
        "(module (memory 1) (func (result i32) i32.const 42))",
    )
    .unwrap();
    let (code, report) = cli(&["type-at", path, "1", "1"]);
    assert_eq!(code, 0);
    assert_eq!(report["mode"], "wat");
    assert_eq!(report["matches"], serde_json::json!([]));
    let (code, report) = cli(&["type-at", path, "999", "1"]);
    assert_eq!(code, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_SOURCE_POSITION");
    std::fs::write(&file, "(module (func $f (result i64) (i32.const 0)))").unwrap();
    let (code, report) = cli(&["type-at", path, "1", "1"]);
    assert_eq!(code, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_TYPE_MISMATCH");
    for args in [
        vec!["type-at", path, "0", "1"],
        vec!["type-at", path, "1"],
        vec!["types", path, "--json"],
    ] {
        assert_eq!(
            Command::new(env!("CARGO_BIN_EXE_wlt"))
                .args(args)
                .output()
                .unwrap()
                .status
                .code(),
            Some(2)
        );
    }
}

#[test]
fn equal_result_types_do_not_hide_distinct_generic_instances() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("instances.wlt");
    let source = "(module (func $id (%type.params $T $U) (param $x $T) (result $T) (local.get $x)) (func $a (result i32) (call (%inst $id i32 i64) (i32.const 1))) (func $b (result i32) (call (%inst $id i32 f64) (i32.const 2))))";
    std::fs::write(&path, source).unwrap();
    let (line, column) = position(source, "(local.get $x)");
    let (_, report) = cli(&["type-at", path.to_str().unwrap(), &line, &column]);
    let matches = report["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2);
    assert!(matches.iter().all(|m| m["ty"] == "i32"));
    assert_ne!(matches[0]["specialization"], matches[1]["specialization"]);
}

#[test]
fn loaded_snapshot_queries_do_not_require_the_file_to_still_exist() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("snapshot.wlt");
    let source = "(module (func $f (result i64) (i64.const 42)))";
    std::fs::write(&file, source).unwrap();
    let loaded = wlt::source::load(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    let (line, column) = position(source, "(i64.const");
    for path in [None, Some(file.as_path())] {
        let query = wlt::inspect::type_at(
            &loaded,
            path,
            line.parse().unwrap(),
            column.parse().unwrap(),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(query.matches[0].ty, "i64");
    }
}
