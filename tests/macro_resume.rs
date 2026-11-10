use std::process::Command;

#[test]
fn composed_expansions_reject_excessive_depth_without_overflowing_the_host_stack() {
    let mut body = "(i32.const 42)".to_string();
    for _ in 0..60 {
        body = format!("(@$wrap {body})");
    }
    let mut template = "?x".to_string();
    for _ in 0..5 {
        template = format!("(block (result i32) {template})");
    }
    let source = format!(
        "(module (@macro $wrap (rule (?x:expr) {template})) (func $f (result i32) {body}))"
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep.walz");
    std::fs::write(&path, source).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_walz"))
        .args(["check", "--json"])
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let diagnostic: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(diagnostic["ok"], false);
    assert_eq!(diagnostic["diagnostics"][0]["code"], "E_MACRO_LIMIT");
}

#[test]
fn one_function_can_expand_512_siblings_without_revisiting_its_prefix() {
    for count in [32, 128, 256, 512] {
        let mut source = String::from(
            "(module (@macro $double (rule (?x:expr) (infer (?T ?x)) (when (Add ?x)) (@let $value (type ?T) ?x (+ $value $value)))) (func $main (export \"main\") (result i64) (@effect pure) (block (result i64)",
        );
        for _ in 1..count {
            source.push_str("(drop (@$double (i64.const 21)))");
        }
        source.push_str("(@$double (i64.const 21)))))");
        let report = walz::macros::report(&source, &Default::default()).unwrap();
        assert_eq!(report.expansions.len(), count);
        assert_eq!(report.checking.module_passes, 1);
        assert_eq!(report.checking.function_checks, 1);
        assert!(
            report.checking.syntax_node_visits < count * 150,
            "{:?}",
            report.checking
        );
        if count == 512 {
            run(&source, "42");
        }
    }
}

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compilation =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wasm");
        std::fs::write(&path, &compilation.wasm).unwrap();
        for (engine, args) in [
            (
                "wasmtime",
                vec![
                    "run",
                    "-W",
                    "gc=y,function-references=y,exceptions=y",
                    "--invoke",
                    "main",
                ],
            ),
            (
                "node",
                vec![
                    "--input-type=module",
                    "-e",
                    "import fs from 'node:fs';const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]));console.log(String(instance.exports.main()));",
                ],
            ),
        ] {
            let output = Command::new(engine).args(args).arg(&path).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
        }
    }
}

#[test]
fn resuming_preserves_outer_locals_labels_and_speculative_type_queries() {
    let source = r#"(module
      (@macro $id (rule (?x:expr) (infer (?T ?x)) (@let $saved (type ?T) ?x $saved)))
      (func $main (export "main") (result i32) (@effect pure)
        (@let $prefix (block $done (result i32) (br $done (i32.const 20)))
          (+ (@$id (block $done (result i32) (br $done (+ $prefix 1))))
             (@$id (@$id (i32.const 21)))))))"#;
    run(source, "42");
    let report = walz::macros::report(source, &Default::default()).unwrap();
    assert_eq!(report.expansions.len(), 3);
    assert_eq!(report.checking.function_checks, 1);
}

#[test]
fn speculative_operator_retry_replays_syntax_without_temporary_local_ids() {
    let source = r#"(module
      (@macro $number (rule () (@let $temporary (i32.const 7) 1)))
      (func $main (export "main") (result i32) (@effect pure)
        (if (result i32) (i32.eq (+ (@$number) (i32.const 41)) (i32.const 42))
          (then (i32.const 42)) (else (i32.const 0)))))"#;
    run(source, "42");
    let report = walz::macros::report(source, &Default::default()).unwrap();
    assert_eq!(report.expansions.len(), 1);
    assert_eq!(report.checking.function_checks, 1);
}

#[test]
fn macro_diagnostics_explain_trailing_repetition_and_whole_body_forwarding() {
    let source = r#"(module
      (@macro $bad (rule ((?value:expr ...) ?body:expr)
        (block ?value ... ?body)))
      (func $f (result i32) (@$bad ((i32.const 1)) (i32.const 42))))"#;
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_MACRO");
    assert!(
        error
            .diagnostic()
            .help
            .iter()
            .any(|h| h.contains("end of its list")),
        "{error}"
    );
    let span = error.diagnostic().primary.as_ref().unwrap().span;
    assert_eq!(&source[span.start..span.end], "...");

    let source = r#"(module
      (@macro $inner (rule (?n:ident ?v:expr ?body:expr)
        (bind ?n ?body) (@let ?n ?v ?body)))
      (@macro $outer (rule (?n:ident ?v:expr ?body:expr)
        (bind ?n ?body) (@$inner ?n ?v (block (result i32) ?body))))
      (func $f (result i32) (@$outer $x (i32.const 42) $x)))"#;
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_MACRO_BINDING");
    assert!(
        error
            .diagnostic()
            .help
            .iter()
            .any(|h| h.contains("unchanged") && h.contains("body")),
        "{error}"
    );
}

#[test]
fn caller_scope_hint_identifies_the_captured_initializer_without_guessing() {
    let header = "(module (@macro $bindings (rule (((?n:ident ?v:expr) ...) ?body:expr) (bind ?n ?body) (@let ((?n ?v) ...) ?body)))";
    for (name, expected_hint) in [("$a", true), ("$unrelated", false)] {
        let source = format!(
            "{header} (func $f (result i32) (@$bindings (($a (i32.const 20)) ($b (+ {name} 22))) $b)))"
        );
        let error = walz::check(&source).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_UNKNOWN_LOCAL");
        assert_eq!(
            error
                .diagnostic()
                .help
                .iter()
                .any(|h| h.contains("caller's scope")),
            expected_hint,
            "{error}"
        );
    }
}
