use std::process::{Command, Output};

fn invoke(source: &str) -> Output {
    let compilation = wlt::compile(source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("macros.wasm");
    std::fs::write(&path, compilation.wasm).unwrap();
    Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            "main",
        ])
        .arg(path)
        .output()
        .unwrap()
}

fn run(source: &str) -> String {
    let output = invoke(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn standard_wat_remains_unchanged_including_stack_syntax_and_data_bytes() {
    let source = r#"(module
      (memory 1) (data (i32.const 0) "\ff\00\22")
      (func (export "main") (result i32) (local i32)
        i32.const 40 local.set 0 local.get 0 i32.const 2 i32.add))"#;
    assert_eq!(run(source), "42");
    assert_eq!(wlt::compile(source).unwrap().wat, source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.wat");
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("check")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn standard_exception_instructions_can_be_used_directly() {
    assert_eq!(
        run(r#"(module (tag $E (param i64))
      (func (export "main") (result i64)
        (block $caught (result i64)
          (try_table (result i64) (catch $E $caught) (throw $E (i64.const 42))))))"#),
        "42"
    );
}

#[test]
fn for_is_a_macro_with_simultaneous_steps_and_an_immutable_induction_binding() {
    let source = r#"(module
      (type $Cell (struct (field $n (mut i64))))
      (func $main (export "main") (result i64)
        (let $cell (struct.new $Cell (i64.const 0))
          (block (result i64)
            (for $walk (bind $i i64 (i64.const 0)) (bind $j i64 (i64.const 1))
              (while (i64.lt_s (local.get $i) (i64.const 3)))
              (step (i64.add (local.get $i) (i64.const 1)) (local.get $i))
              (struct.set $Cell $n (local.get $cell)
                (i64.add (struct.get $Cell $n (local.get $cell)) (local.get $j))))
            (i64.add (struct.get $Cell $n (local.get $cell)) (i64.const 40))))))"#;
    assert_eq!(run(source), "42");
    let wat = wlt::compile(source).unwrap().wat;
    assert!(!wat.contains("(for "));
    assert!(wat.contains("(loop ") && wat.contains("(br "));
}

#[test]
fn try_catch_uses_native_tags_and_supports_multiple_and_gc_payloads() {
    let source = r#"(module
      (type $Box (struct (field $n i64)))
      (tag $Unused (param i32)) (tag $Error (param i32 (ref $Box)))
      (func $raise (result i64)
        (throw $Error (i32.const 2) (struct.new $Box (i64.const 40))))
      (func $main (export "main") (result i64)
        (try (result i64) (do (call $raise))
          (catch $Unused ($code) (i64.const -1))
          (catch $Error ($code $box)
            (i64.add (i64.extend_i32_s (local.get $code)) (struct.get $Box $n (local.get $box)))))))"#;
    assert_eq!(run(source), "42");
    let wat = wlt::compile(source).unwrap().wat;
    assert!(wat.contains("try_table") && wat.contains("throw $Error"));
    assert!(!wat.contains("(try "));
}

#[test]
fn normal_return_does_not_run_handlers_and_rethrow_escapes_the_current_handler() {
    assert_eq!(
        run(r#"(module (tag $E (param i64))
      (func $main (export "main") (result i64)
        (i64.add
          (try (result i64) (do (i64.const 20)) (catch_all (unreachable)))
          (try (result i64)
            (do (try (result i64) (do (throw $E (i64.const 21)))
              (catch $E ($n) (throw $E (i64.add (local.get $n) (i64.const 1))))))
            (catch $E ($n) (local.get $n))))))"#),
        "42"
    );
}

#[test]
fn catch_all_catches_exceptions_but_does_not_catch_traps() {
    assert_eq!(
        run(r#"(module (tag $E)
      (func $main (export "main") (result i64)
        (try (result i64) (do (throw $E)) (catch_all (i64.const 42)))))"#),
        "42"
    );
    let output = invoke(
        r#"(module (func $main (export "main") (result i64)
      (try (result i64) (do (i64.div_s (i64.const 1) (i64.const 0)))
        (catch_all (i64.const 42)))))"#,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("divide by zero"));
}

#[test]
fn exceptions_and_for_keep_evaluation_order_and_work_inside_tests() {
    assert_eq!(
        run(r#"(module
      (tag $E (param i64))
      (type $Cell (struct (field $n (mut i64))))
      (func $main (export "main") (result i64)
        (let $cell (struct.new $Cell (i64.const 0))
          (try (result i64)
            (do (block (result i64)
              (for $walk (bind $i i64 (i64.const 0))
                (while (i64.lt_s (local.get $i) (i64.const 10)))
                (step (i64.add (local.get $i) (i64.const 1)))
                (if (i64.eq (local.get $i) (i64.const 3))
                  (then (throw $E (i64.const 42)))
                  (else (struct.set $Cell $n (local.get $cell) (local.get $i)))))
              (i64.const -1)))
            (catch $E ($n)
              (i64.add (local.get $n) (i64.sub (struct.get $Cell $n (local.get $cell)) (i64.const 2))))))))"#),
        "42"
    );
}

#[test]
fn malformed_macros_and_payload_types_are_rejected() {
    for (source, message) in [
        ("(module (tag $E (result i32)))", "tag payload"),
        ("(module (tag $E) (tag $E))", "duplicate tag"),
        (
            "(module (tag $E (param i64)) (func $f (throw $E (i32.const 1))))",
            "expected i64, got i32",
        ),
        ("(module (func $f (throw $Missing)))", "unknown tag"),
        (
            "(module (tag $E (param i64)) (func $f (try (do (nop)) (catch $E () (nop)))))",
            "payload binding count",
        ),
        (
            "(module (tag $E) (func $f (try (do (nop)) (catch $E () (nop)) (catch $E () (nop)))))",
            "duplicate catch",
        ),
        (
            "(module (func $f (try (do (nop)) (catch_all (nop)) (catch_all (nop)))))",
            "catch_all must be last",
        ),
        (
            "(module (func $f (for $l (bind $i i64 (i64.const 0)) (while (i64.const 1)) (step (i64.const 1)) (nop))))",
            "expected i32, got i64",
        ),
        (
            "(module (func $f (for $l (bind $i i64 (i64.const 0)) (while (i32.const 1)) (step (i32.const 1)) (nop))))",
            "expected i64, got i32",
        ),
        (
            "(module (func $f (for $l (bind $i i64 (i64.const 0)) (while (i32.const 1)) (step (i64.const 1)) (i64.const 1))))",
            "expected unit, got i64",
        ),
    ] {
        let error = wlt::check(source).unwrap_err().to_string();
        assert!(error.contains(message), "{source}\n{error}");
    }
}

#[test]
fn opaque_native_wat_is_validated_but_never_assumed_pure() {
    let source = r#"(module
      (import "host" "f" (func (param i64) (result i32)))
      (func (export "pair") (param i32) (result i64 i32) i64.const 42 local.get 0))"#;
    let compilation = wlt::compile(source).unwrap();
    assert!(compilation.ir.is_none() && compilation.analysis.opaque);
    let effects = serde_json::to_value(compilation.analysis.module_effects).unwrap();
    assert!(
        effects
            .as_object()
            .unwrap()
            .values()
            .all(|value| value == true)
    );
    let types = wlt::native::types(&compilation.wasm).unwrap();
    assert_eq!(types.functions[0].params, ["i64"]);
    assert_eq!(types.functions[0].results, ["i32"]);
    assert_eq!(types.functions[1].params, ["i32"]);
    assert_eq!(types.functions[1].results, ["i64", "i32"]);
    assert_eq!(types.functions[1].exports, ["pair"]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.wat");
    std::fs::write(&path, source).unwrap();
    for command in ["types", "analyze", "ir", "expand"] {
        let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
            .arg(command)
            .arg(&path)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn native_wat_can_use_standard_names_and_structural_types() {
    let source = r#"(module
      (type $A (struct)) (type $B (struct))
      (func $__user (export "main") (result i32)
        (drop (block (result (ref $A)) (struct.new $B)))
        (i32.const 42)))"#;
    assert_eq!(run(source), "42");
    assert_eq!(wlt::expand(source).unwrap(), source);
}

#[test]
fn exception_effects_are_distinct_from_traps_and_propagate_through_calls() {
    let module = wlt::check(
        r#"(module (tag $E)
      (func $trap (result i64) (i64.div_s (i64.const 1) (i64.const 0)))
      (func $raise (throw $E))
      (func $caught (try (do (call $raise)) (catch_all (nop)))))"#,
    )
    .unwrap();
    let analysis = wlt::analysis::analyze(&module);
    assert!(analysis.functions[0].effects.may_trap);
    assert!(!analysis.functions[0].effects.may_throw);
    assert!(analysis.functions[1].effects.may_throw);
    assert!(analysis.functions[2].effects.may_throw);
    assert_eq!(analysis.functions[2].calls, ["$raise"]);
    assert!(!analysis.functions[2].effects.host);
}

#[test]
fn empty_for_skips_body_and_step_and_nested_handlers_keep_lexical_scopes() {
    assert_eq!(
        run(r#"(module (tag $E (param i64))
      (func $main (export "main") (result i64)
        (block (result i64)
          (for $empty (bind $i i64 (i64.const 0))
            (while (i32.const 0)) (step (unreachable)) (unreachable))
          (loop $walk (bind $n i64 (i64.const 0)) (result i64)
            (try (result i64) (do (throw $E (local.get $n)))
              (catch $E ($n)
                (if (result i64) (i64.lt_s (local.get $n) (i64.const 42))
                  (then (recur $walk (i64.add (local.get $n) (i64.const 1))))
                  (else (local.get $n)))))))))"#),
        "42"
    );
}
