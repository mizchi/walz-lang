use std::process::Command;
const SOURCE: &str = r#"(module
  (%macro $twice (rule (?x:expr) (+ ?x ?x)))
  (%macro $once (rule (?x:expr) (%let $saved ?x (+ $saved $saved))))
  (%macro $drop (rule (?x:expr) (i32.const 0)))
  (%macro $loop (rule (?x:expr) (%loop $again (bind $n i32 0)
    (if (< $n 2) (then (block (drop ?x) (%recur $again (+ $n 1)))) (else (nop))))))
  (func $f (result i32) (block (result i32)
    (drop (%$twice (i32.const 1)))
    (drop (%$once (i32.const 2)))
    (%$loop (i32.const 3))
    (%$drop (i32.const 4)))))"#;

#[test]
fn report_distinguishes_syntax_usage_from_runtime_evaluations() {
    let report = wlt::macros::report(SOURCE, &Default::default()).unwrap();
    let json = serde_json::to_value(&report).unwrap();
    for (i, kind, count) in [
        (0, "duplicated", 2),
        (1, "single", 1),
        (2, "single", 1),
        (3, "discarded", 0),
    ] {
        let input = &json["expansions"][i]["inputs"][0];
        assert_eq!(input["usage"], kind);
        assert_eq!(input["outputs"].as_array().unwrap().len(), count);
    }
}

#[test]
fn cli_has_a_readable_usage_view_and_keeps_default_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.wlt");
    std::fs::write(&path, SOURCE).unwrap();
    let json = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("macro-expand")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        json.status.success(),
        "{}",
        String::from_utf8_lossy(&json.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(json["expansions"].as_array().unwrap().len(), 4);
    let out = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["macro-expand", "--text"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    for required in [
        "duplicated (2 syntax occurrences)",
        "discarded (0 syntax occurrences)",
        "single (1 syntax occurrence)",
        "runtime evaluation depends on control flow",
        "usage.wlt:",
    ] {
        assert!(text.contains(required), "missing {required}: {text}");
    }
}

#[test]
fn lsp_expansion_returns_usage_and_structured_types() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let uri = wlt::lsp::file_uri(&dir.path().join("main.wlt")).unwrap();
    let source = "(module (%macro $m (rule (?x:expr) (infer (?T ?x)) (+ ?x ?x))) (func $f (result i64) (%$m (i64.const 1))))";
    let mut session = wlt::lsp::Session::new(Default::default());
    session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"text":source,"version":1}}}));
    let response=session.handle(json!({"id":1,"method":"wlt/expandMacro","params":{"textDocument":{"uri":uri},"position":{"line":0,"character":source.find("%$m").unwrap()}}}));
    let expansion = &response[0]["result"]["expansions"][0];
    assert_eq!(
        expansion["inputs"][0]["usage"], "duplicated",
        "{response:?}"
    );
    assert_eq!(expansion["types"]["?T"]["name"], "i64");
}
