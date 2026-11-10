fn same_wasm(short: &str, explicit: &str) {
    assert_eq!(
        walz::compile(short).unwrap().wasm,
        walz::compile(explicit).unwrap().wasm
    );
}

#[test]
fn constructors_use_their_expected_value_type() {
    same_wasm(
        r#"(module (func $f (result (@option (@result (@tuple i64 i32) i32)))
          (@option.some (@result.ok (@tuple.new (i64.const 40) (i32.const 2))))))"#,
        r#"(module (func $f (result (@option (@result (@tuple i64 i32) i32)))
          (@option.some (@result (@tuple i64 i32) i32)
            (@result.ok (@tuple i64 i32) i32 (@tuple.new i64 i32 (i64.const 40) (i32.const 2))))))"#,
    );
    for (ty, expression) in [
        ("(@option i64)", "(@option.none)"),
        ("(@result @unit i32)", "(@result.ok)"),
        ("(@result i64 i32)", "(@result.err (i32.const 1))"),
        ("(@box i64)", "(@box.new (i64.const 42))"),
    ] {
        walz::compile(&format!("(module (func $f (result {ty}) {expression}))")).unwrap();
    }
}

#[test]
fn expected_types_reach_arguments_fields_and_annotated_bindings() {
    walz::compile(
        r#"(module
      (type $Box (struct (field (@option i64))))
      (func $take (param $v (@option i64)) (drop (local.get $v)))
      (func $f
        (@let $v (@option i64) (@option.some (i64.const 42))
          (block (call $take (@option.none))
            (drop (struct.new $Box (@option.some (i64.const 1))))))))"#,
    )
    .unwrap();
}

#[test]
fn missing_or_wrong_constructor_context_is_rejected() {
    for expression in [
        "(@let $v (@option.none) (nop))",
        "(@let $v (@result.ok (i64.const 1)) (nop))",
        "(@let $v (@tuple.new (i64.const 1) (i32.const 2)) (nop))",
        "(@let $v (@box.new (i64.const 1)) (@box.close (@move $v)))",
    ] {
        let error = walz::check(&format!("(module (func $f {expression}))"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("annotation"), "{error}");
    }
    let error =
        walz::check("(module (func $f (result (@option i64)) (@option.some (i32.const 1))))")
            .unwrap_err()
            .to_string();
    assert!(error.contains("type mismatch"), "{error}");
}

#[test]
fn pair_patterns_lower_to_one_match_and_consume_owned_payloads() {
    same_wasm(
        r#"(module (func $f (param $p (@tuple i64 i32)) (result i64)
          (@let ($a $b) (local.get $p) (i64.add (local.get $a) (i64.extend_i32_s (local.get $b))))))"#,
        r#"(module (func $f (param $p (@tuple i64 i32)) (result i64)
          (@match (@tuple i64 i32) (result i64) (local.get $p)
            (@case pair ($a $b) (i64.add (local.get $a) (i64.extend_i32_s (local.get $b)))))))"#,
    );
    walz::compile(
        r#"(module (func $f (param $p (@tuple @unit (@box i64)))
      (@let ($value) (@move $p) (@box.close (@move $value)))))"#,
    )
    .unwrap();
    for body in [
        "(@let ($a $a) (local.get $p) (nop))",
        "(@let ($a) (local.get $p) (nop))",
    ] {
        assert!(
            walz::check(&format!(
                "(module (func $f (param $p (@tuple i64 i32)) {body}))"
            ))
            .is_err()
        );
    }
    let error = walz::check(
        r#"(module (func $f (param $p (@tuple (@box i64) i32))
      (@let ($a $b) (local.get $p) (@box.close (@move $a)))))"#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("move"), "{error}");
}

#[test]
fn short_result_let_uses_subject_error_and_expected_or_body_success() {
    walz::compile(
        r#"(module
      (func $f (param $r (@result i64 i32)) (result (@result i32 i32))
        (@result.let ($v) (local.get $r) (@result.ok (i32.wrap_i64 (local.get $v)))))
      (func $g (param $r (@result i64 i32))
        (@let $mapped (@result.let ($v) (local.get $r) (@result.ok (local.get $v)))
          (drop (local.get $mapped)))))"#,
    )
    .unwrap();
    walz::compile(
        r#"(module (func $f (param $r (@result @unit @unit)) (result (@result @unit @unit))
      (@result.let () (local.get $r) (@result.ok))))"#,
    )
    .unwrap();
    let error = walz::check(
        r#"(module (func $f (param $r (@result i64 i32))
      (@let $v (@result.let ($v) (local.get $r) (@result.err (i32.const 1))) (nop))))"#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("annotation"), "{error}");
}

#[test]
fn short_fold_infers_state_and_result_without_leaking_probe_locals() {
    let module = walz::check(
        r#"(module (func $f (result (@result (@tuple i64 i32) i32))
      (@stream.fold-result
        (@let $source (@stream.empty (@result i64 i32)) (@move $source))
        (@let $initial (i64.const 0)
          (block (result (@tuple i64 i32))
            (@loop $probe (bind $temporary i32 (i32.const 0)) (nop))
            (@tuple.new (local.get $initial) (i32.const 0)))) ($state $item)
        (@result.ok (local.get $state)))))"#,
    )
    .unwrap();
    assert_eq!(
        module.functions[0]
            .locals
            .iter()
            .filter(|b| b.name == "$source")
            .count(),
        1
    );
    for name in ["$initial", "$temporary"] {
        assert_eq!(
            module.functions[0]
                .locals
                .iter()
                .filter(|b| b.name == name)
                .count(),
            1
        );
    }
    let temporary = module.functions[0]
        .locals
        .iter()
        .find(|b| b.name == "$temporary")
        .unwrap();
    assert!(matches!(
        temporary.kind,
        walz::ir::BindingKind::Loop(walz::ir::LoopId(1))
    ));
    walz::compile(
        r#"(module (func $f
      (@let $r (@stream.fold-result (@stream.empty (@result i64 i32)) (i64.const 0)
        ($sum $item) (@result.ok (i64.add (local.get $sum) (local.get $item))))
        (drop (local.get $r)))))"#,
    )
    .unwrap();
}

#[test]
fn short_match_uses_subject_and_expected_output_types() {
    same_wasm(
        r#"(module (func $f (param $v (@option i64)) (result i64)
          (@match (local.get $v) (@case none () (i64.const 0)) (@case some ($n) (local.get $n)))))"#,
        r#"(module (func $f (param $v (@option i64)) (result i64)
          (@match (@option i64) (result i64) (local.get $v)
            (@case none () (i64.const 0)) (@case some ($n) (local.get $n)))))"#,
    );
    walz::compile(
        r#"(module (type $V (@variant (no) (yes i64)))
      (func $f (param $v (ref $V)) (result (@option i64))
        (@match (local.get $v) (@case no () (@option.none))
          (@case yes ($n) (@option.some (local.get $n))))))"#,
    )
    .unwrap();
    let error = walz::check(
        r#"(module (func $f (param $v (@option i64))
      (@let $n (@match (local.get $v) (@case none () (i64.const 0))
        (@case some ($n) (local.get $n))) (nop))))"#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("annotation"), "{error}");
}

#[test]
fn expected_types_reach_storage_exception_and_loop_operands() {
    walz::compile(
        r#"(module
      (type $Box (struct (field (mut (@option i64)))))
      (type $Array (array (mut (@option i64))))
      (tag $Stop)
      (func $f (param $box (ref $Box))
        (@let $array (array.new $Array (@option.none) (i32.const 2))
          (block
            (struct.set $Box 0 (local.get $box) (@option.some (i64.const 1)))
            (array.set $Array (local.get $array) (i32.const 0) (@option.some (i64.const 2)))
            (drop (array.new_fixed $Array 1 (@option.none)))
            (drop (@try (result (@option i64)) (do (@option.none))
              (catch $Stop () (@option.some (i64.const 3)))))
            (@loop $loop (bind $state (@option i64) (@option.none))
              (if (i32.const 0)
                (then (@recur $loop (@option.some (i64.const 4)))) (else (nop))))))))"#,
    )
    .unwrap();
    for source in [
        "(module (func $f (@result.ok)))",
        "(module (func $f (result (@option i64)) (block (@option.none))))",
        "(module (func $f (result (@option i64)) (if (i32.const 1) (then (@option.none)) (else (@option.none)))))",
    ] {
        assert!(walz::check(source).is_err(), "{source}");
    }
}

#[test]
fn short_macros_preserve_cleanup_and_error_execution() {
    let explicit = include_str!("scoped-macros.test.walz");
    let short = explicit
        .replace("stream.fold-result i64 i64 i32", "stream.fold-result")
        .replace("stream.fold-result unit i64 unit", "stream.fold-result")
        .replace("        (@result @unit i32)\n        i64\n", "")
        .replace("          (@result i64 i32)\n          i64\n", "")
        .replace("        (@result i64 i32)\n        i64\n", "")
        .replace("(@result.ok i64 i32 (i64.add", "(@result.ok (i64.add");
    same_wasm(&short, explicit);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("readable.test.walz");
    std::fs::write(&path, short).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("test")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("8 passed; 0 failed; 0 errors"));
}

#[test]
fn short_macros_preserve_ownership_and_effect_boundaries() {
    for source in [
        r#"(module (func $f (param $r (@result i64 i32)) (result (@result i64 i32))
          (@let $owner (@future.ready i64 (i64.const 0))
            (@result.let ($v) (local.get $r)
              (block (result (@result i64 i32)) (@future.close i64 (@move $owner)) (@result.ok (local.get $v)))))))"#,
        r#"(module (func $f (result (@result i64 i32)) (@effect pure)
          (@stream.fold-result (@stream.empty (@result i64 i32)) (i64.const 0) ($s $i)
            (@result.ok (local.get $s)))))"#,
        r#"(module (func $f (param $r (@result i64 (@box i32))) (result (@result i64 (@box i32)))
          (@result.let ($v) (@move $r) (@result.ok (local.get $v)))))"#,
        r#"(module (func $f (result (@result (@box i64) i32))
          (@stream.fold-result (@stream.empty (@result i64 i32)) (@box.new i64 (i64.const 0)) ($s $i)
            (@result.ok (@move $s)))))"#,
    ] {
        assert!(walz::check(source).is_err(), "{source}");
    }
    walz::compile(
        r#"(module (func $f (param $r (@result (@box i64) i32)) (result (@result (@box i64) i32))
      (@result.let ($v) (@move $r) (@result.ok (@move $v)))))"#,
    )
    .unwrap();
}

#[test]
fn result_propagation_keeps_the_exact_error_type() {
    for (input, output) in [
        ("(ref null $Error)", "(ref $Error)"),
        ("(ref $Error)", "(ref null $Error)"),
    ] {
        let source = format!(
            r#"(module (type $Error (struct))
          (func $f (param $r (@result i64 {input})) (result (@result i64 {output}))
            (@result.let ($v) (local.get $r) (@result.ok (local.get $v)))))"#
        );
        let error = walz::check(&source).unwrap_err().to_string();
        assert!(error.contains("type mismatch"), "{error}");
    }
}

#[test]
fn inferred_composite_types_survive_checking_and_emission() {
    walz::compile(
        r#"(module
      (func $fold
        (@let $r (@stream.fold-result (@stream.empty (@result i64 i32))
          (@tuple.new i32 i64 (i32.const 0) (i64.const 0)) ($s $i)
          (@result.ok (local.get $s))) (drop (local.get $r))))
      (func $map (param $input (@result i64 i32)) (result i32)
        (@let $r (@result.let ($v) (local.get $input) (@result.ok (i32.wrap_i64 (local.get $v))))
          (@match (local.get $r) (@case ok ($value) (local.get $value))
            (@case err ($error) (local.get $error))))))"#,
    )
    .unwrap();
    let error = walz::check(
        r#"(module (func $f (param $input (@result i64 i32))
      (@let $owner (@box.new i64 (i64.const 0))
        (block (@result.let ($v) (local.get $input) (@result.ok (@move $owner))) (nop)))))"#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("cannot discard an owned value"), "{error}");
}

#[test]
fn fold_evaluates_source_then_initial_once() {
    let short = r#"(module
      (type $Counter (struct (field (mut i32))))
      (func $main (export "main") (result i32)
        (@let $counter (struct.new $Counter (i32.const 0))
          (@match (@result i64 i32) (result i32)
            (@stream.fold-result
              (block (result (@stream (@result i64 i32)))
                (struct.set $Counter 0 (local.get $counter)
                  (i32.add (i32.mul (struct.get $Counter 0 (local.get $counter)) (i32.const 10)) (i32.const 1)))
                (@stream.empty (@result i64 i32)))
              (block (result i64)
                (struct.set $Counter 0 (local.get $counter)
                  (i32.add (i32.mul (struct.get $Counter 0 (local.get $counter)) (i32.const 10)) (i32.const 2)))
                (i64.const 0))
              ($state $item) (@result.ok (local.get $state)))
            (@case ok ($value) (struct.get $Counter 0 (local.get $counter)))
            (@case err ($error) (i32.const -1))))))"#;
    let explicit = short
        .replace("stream.fold-result\n", "stream.fold-result i64 i64 i32\n")
        .replace(
            "(@result.ok (local.get $state))",
            "(@result.ok i64 i32 (local.get $state))",
        );
    same_wasm(short, &explicit);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("order.wasm");
    std::fs::write(&path, walz::compile(short).unwrap().wasm).unwrap();
    let output = std::process::Command::new("wasmtime")
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
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "12");
}
