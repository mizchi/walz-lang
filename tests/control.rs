use std::process::Command;
fn run(body: &str, expected: &str) {
    let source =
        format!("(module (func $main (export \"main\") (result i32) (@effect pure) {body}))");
    let compiled = walz::compile(&source).unwrap();
    assert!(!compiled.analysis.opaque);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    let out = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            "main",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), expected);
}
#[test]
fn typed_returns_and_labeled_branches_keep_native_wat_order_and_results() {
    run(
        "(block (result i32) (return (i32.const 42)) (i32.const 0))",
        "42",
    );
    run(
        "(block $done (result i32) (br $done (i32.const 42)) (i32.const 0))",
        "42",
    );
    run(
        "(block $done (result i32) (i32.add (br_if $done (i32.const 40) (i32.const 0)) (i32.const 2)))",
        "42",
    );
    run(
        "(block $done (result i32) (drop (br_if $done (i32.const 42) (i32.const 1))) (i32.const 0))",
        "42",
    );
    run(
        "(block $outer (result i32) (block $inner (br $outer (i32.const 42))) (i32.const 0))",
        "42",
    );
}
#[test]
fn early_exits_cannot_discard_owners_or_cross_mismatched_label_types() {
    for body in [
        "(return (i64.const 42))",
        "(block $done (result i32) (br $done (i64.const 42)))",
        "(br $unknown)",
    ] {
        assert!(walz::check(&format!("(module (func $f (result i32) {body}))")).is_err());
    }
    let bad = "(module (func $f (param $x (@box i32)) (result i32) (@effect pure) (return (i32.const 42))))";
    assert!(walz::check(bad).is_err());
    let good = "(module (func $f (param $x (@box i32)) (result (@box i32)) (@effect pure) (return (@move $x))))";
    walz::check(good).unwrap();
}

#[test]
fn named_loop_branches_and_owned_block_results_are_checked() {
    run(
        "(@loop $again (bind $n i32 (i32.const 0)) (result i32) (if (result i32) (i32.lt_s (local.get $n) (i32.const 42)) (then (br $again (i32.add (local.get $n) (i32.const 1)))) (else (local.get $n))))",
        "42",
    );
    let good = r#"(module (func $main (export "main") (result i32) (@effect pure)
      (@let $x (@box.new i32 (i32.const 42))
        (@match (@box i32) (result i32)
          (block $done (result (@box i32)) (br $done (@move $x)))
          (@case own ($n) (return (local.get $n)))))))"#;
    let compiled = walz::verified::compile(good).unwrap();
    assert!(!compiled.analysis.opaque);
    let bad = r#"(module (func $f (result i32)
      (block $done (result i32) (@let $x (@box.new i32 (i32.const 1)) (br $done (i32.const 42))))))"#;
    let error = walz::check(bad).unwrap_err().to_string();
    assert!(error.contains("unconsumed"), "{error}");
}
#[test]
fn typed_source_accepts_empty_result_vectors_without_erasing_validation() {
    for body in [
        "(block (result) (nop))",
        "(loop $again (result) (nop))",
        "(if (result) (i32.const 1) (then (nop)) (else (nop)))",
    ] {
        let source = format!(
            r#"(module (@use "walz:core" (type $unit))
              (func $step (result) {body})
              (func $entry (export "f") (result i32)
                (block (result i32) (call $step) (i32.const 42))))"#
        );
        let compiled = walz::compile(&source).unwrap();
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&compiled.wasm)
            .unwrap();
        assert!(walz::check(&source.replace("(nop)", "(i64.const 1)")).is_err());
    }
}
