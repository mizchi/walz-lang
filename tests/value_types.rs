use std::process::Command;

fn run(source: &str) -> String {
    let compilation = gcwat::compile(source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("values.wasm");
    std::fs::write(&path, compilation.wasm).unwrap();
    let output = Command::new("wasmtime")
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
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn option_and_result_are_instantiated_and_nested_as_value_types() {
    assert_eq!(
        run(r#"(module
      (func $make (result (option (result i64 i32))) (effects pure)
        (option.some (result i64 i32) (result.ok i64 i32 (i64.const 42))))
      (func $main (export "main") (result i64) (effects pure)
        (match (option (result i64 i32)) (result i64) (call $make)
          (case none () (i64.const -1))
          (case some ($result)
            (match (result i64 i32) (result i64) (local.get $result)
              (case ok ($value) (local.get $value))
              (case err ($code) (i64.const -2)))))))"#),
        "42"
    );
}

#[test]
fn unit_results_and_none_are_distinct_cases() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i64)
        (i64.add
          (match (result unit i32) (result i64) (result.ok unit i32)
            (case ok () (i64.const 40)) (case err ($code) (i64.const -1)))
          (match (option i64) (result i64) (option.none i64)
            (case none () (i64.const 2)) (case some ($n) (local.get $n))))))"#),
        "42"
    );
}

#[test]
fn value_types_work_in_fields_arrays_and_explicit_captures() {
    assert_eq!(
        run(r#"(module
      (type $Values (array (option i64)))
      (type $Box (struct (field (result i64 i32))))
      (closure $Read (capture $value (option i64)) (result i64) (effects pure)
        (match (option i64) (result i64) (local.get $value)
          (case none () (i64.const 0)) (case some ($n) (local.get $n))))
      (func $main (export "main") (result i64)
        (let $values (array.new_fixed $Values 1 (option.some i64 (i64.const 40)))
          (let $box (struct.new $Box (result.ok i64 i32 (i64.const 2)))
            (i64.add
              (closure.call $Read (closure.new $Read (array.get $Values (local.get $values) (i32.const 0))))
              (match (result i64 i32) (result i64) (struct.get $Box 0 (local.get $box))
                (case ok ($n) (local.get $n)) (case err ($e) (i64.const 0))))))))"#),
        "42"
    );
}

#[test]
fn value_storage_identity_is_hidden_but_payload_null_is_preserved() {
    let source = r#"(module (type $T (struct))
      (func $main (export "main") (result i32)
        (match (option (ref null $T)) (result i32)
          (option.some (ref null $T) (ref.null $T))
          (case none () (i32.const 0))
          (case some ($reference) (ref.is_null (local.get $reference))))))"#;
    assert_eq!(run(source), "1");
    let error = gcwat::check(
        "(module (func $f (result i32) (ref.eq (option.none i64) (option.none i64))))",
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("ref.eq needs references"), "{error}");
}

#[test]
fn value_types_and_contracts_are_visible_in_type_and_effect_reports() {
    let source =
        "(module (func $f (result (option i64)) (effects pure) (option.some i64 (i64.const 42))))";
    let module = gcwat::check(source).unwrap();
    assert_eq!(
        module.type_name(&module.functions[0].result),
        "(option i64)"
    );
    let info = gcwat::compile(source).unwrap();
    let report = serde_json::to_value(info.analysis).unwrap();
    assert_eq!(
        report["functions"][0]["semantic_effects"]["allocates"],
        false
    );
    assert_eq!(report["functions"][0]["effects"]["allocates"], true);
    assert_eq!(report["functions"][0]["effects"]["may_trap"], true);
    assert_eq!(
        report["functions"][0]["declared_effects"]["allocates"],
        false
    );
}

#[test]
fn wrong_payloads_and_incomplete_matches_are_rejected() {
    for (source, message) in [
        (
            "(module (func $f (result (option i64)) (option.some i64 (i32.const 1))))",
            "expected i64, got i32",
        ),
        (
            "(module (func $f (result (option i64)) (option.none i32)))",
            "expected (option i64), got (option i32)",
        ),
        (
            "(module (func $f (result (result i64 i32)) (result.err i64 i32 (i64.const 1))))",
            "expected i32, got i64",
        ),
        (
            "(module (func $f (match (option i64) (option.none i64) (case none () (nop)))))",
            "non-exhaustive match",
        ),
        (
            "(module (func $f (result (result unit i32)) (result.ok unit i32 (i64.const 1))))",
            "argument count",
        ),
        (
            "(module (func $f (result unit) (nop)))",
            "expected numeric type",
        ),
    ] {
        let error = gcwat::check(source).unwrap_err().to_string();
        assert!(error.contains(message), "{source}\n{error}");
    }
}
