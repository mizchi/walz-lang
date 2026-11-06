use std::process::Command;

fn run(source: &str, expected: &str) -> wlt::Compilation {
    let compiled = wlt::compile(source).unwrap();
    assert!(!compiled.analysis.opaque, "SIMD must use the checked core");
    assert!(compiled.ir.is_some());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("simd.wasm");
    std::fs::write(&path, &compiled.wasm).unwrap();
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
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    compiled
}

#[test]
fn typed_simd_covers_vector_shapes_and_mixed_scalar_operands() {
    for (ty, expression, expected) in [
        (
            "i32",
            "(i32x4.extract_lane 0 (i32x4.add (v128.const i32x4 40 0 0 0) (i32x4.splat (i32.const 2))))",
            "42",
        ),
        (
            "i32",
            "(i8x16.extract_lane_u 15 (i8x16.splat (i32.const 255)))",
            "255",
        ),
        (
            "i32",
            "(i16x8.extract_lane_s 7 (i16x8.splat (i32.const 65535)))",
            "-1",
        ),
        (
            "i64",
            "(i64x2.extract_lane 1 (i64x2.shl (i64x2.splat (i64.const 21)) (i32.const 1)))",
            "42",
        ),
        (
            "f32",
            "(f32x4.extract_lane 2 (f32x4.mul (f32x4.splat (f32.const 3)) (f32x4.splat (f32.const 14))))",
            "42",
        ),
        (
            "f64",
            "(f64x2.extract_lane 1 (f64x2.add (v128.const f64x2 40 40) (f64x2.splat (f64.const 2))))",
            "42",
        ),
        (
            "i64",
            "(i64x2.extract_lane 1 (i64x2.replace_lane 1 (v128.const i64x2 0 0) (i64.const 42)))",
            "42",
        ),
        (
            "i32",
            "(v128.any_true (v128.and (v128.const i32x4 1 0 0 0) (v128.const i32x4 1 0 0 0)))",
            "1",
        ),
        ("i32", "(i32x4.bitmask (v128.const i32x4 -1 0 -1 0))", "5"),
        (
            "i32",
            "(i32x4.extract_lane 0 (v128.bitselect (i32x4.splat (i32.const 42)) (i32x4.splat (i32.const 0)) (i32x4.splat (i32.const -1))))",
            "42",
        ),
        (
            "i32",
            "(i32x4.extract_lane 0 (i8x16.shuffle 16 17 18 19 0 1 2 3 4 5 6 7 8 9 10 11 (v128.const i32x4 1 2 3 4) (v128.const i32x4 42 8 9 10)))",
            "42",
        ),
        (
            "i32",
            "(i32x4.extract_lane 0 (i32x4.trunc_sat_f32x4_s (f32x4.splat (f32.const nan))))",
            "0",
        ),
    ] {
        let source = format!(
            "(module (func $main (export \"main\") (result {ty}) (%effects pure) (%let $answer {expression} (local.get $answer))))"
        );
        let module = run(&source, expected);
        let main = &module.analysis.functions[0];
        assert!(
            !main.semantic_effects.reads && !main.semantic_effects.writes && !main.effects.may_trap
        );
    }
}

#[test]
fn scalar_bit_counts_use_standard_width_and_zero_semantics() {
    for (ty, op, value, expected) in [
        ("i32", "popcnt", "-1", "32"),
        ("i32", "clz", "0", "32"),
        ("i32", "ctz", "0", "32"),
        ("i32", "ctz", "-2147483648", "31"),
        ("i64", "popcnt", "-1", "64"),
        ("i64", "clz", "0", "64"),
        ("i64", "ctz", "0", "64"),
        ("i64", "ctz", "-9223372036854775808", "63"),
    ] {
        run(
            &format!(
                "(module (func $main (export \"main\") (result {ty}) (%effects pure) ({ty}.{op} ({ty}.const {value}))))"
            ),
            expected,
        );
    }
    let error = wlt::compile(
        "(module (%law \"bit count\" (i32.eq (i32.popcnt (i32.const -1)) (i32.const 32))))",
    )
    .err()
    .unwrap();
    assert!(
        error.to_string().contains("unsupported law operation"),
        "{error}"
    );
}

#[test]
fn vectors_are_shared_values_in_gc_storage_captures_generics_and_lists() {
    let source = r#"(module
      (%import "wlt:std" (type $option) (func $list-builder.new) (func $list-builder.push) (func $list-builder.finish))
      (type $Box (struct (field $v (mut v128))))
      (type $Array (array (mut v128)))
      (func $id (%type.params $T) (%where (shared $T) (nonunit $T)) (param $x $T) (result $T) (%effects pure) (local.get $x))
      (%closure $Add (%capture $base v128) (param $x v128) (result v128) (%effects pure) (i32x4.add (local.get $base) (local.get $x)))
      (func $main (export "main") (result i32) (%effects reads writes allocates)
        (%let (($box (struct.new $Box (v128.const i32x4 0 0 0 0)))
               ($array (array.new_fixed $Array 1 (v128.const i32x4 0 0 0 0)))
               ($builder (call (%inst $list-builder.new v128))))
          (block (result i32)
            (struct.set $Box $v (local.get $box) (i32x4.splat (i32.const 40)))
            (array.set $Array (local.get $array) (i32.const 0) (i32x4.splat (i32.const 2)))
            (%let $f (%closure.new $Add (struct.get $Box $v (local.get $box)))
              (block (result i32)
                (call (%inst $list-builder.push v128) (local.get $builder)
                  (call (%inst $id v128) (%closure.call $Add (local.get $f) (array.get $Array (local.get $array) (i32.const 0)))))
                (%let $values (call (%inst $list-builder.finish v128) (local.get $builder))
                  (%match (%inst $option v128) (result i32) (%option.some v128 (%list.get v128 (local.get $values) (i32.const 0)))
                    (case none () (i32.const 0))
                    (case some ($v) (i32x4.extract_lane 0 (local.get $v)))))))))))"#;
    run(source, "42");
    wlt::component::compile(source).unwrap();
    wlt::component::core_js::compile(source).unwrap();
}

#[test]
fn simd_operands_keep_left_to_right_evaluation_and_effects() {
    let source = r#"(module
      (type $Counter (struct (field $n (mut i32))))
      (func $tick (param $c (ref $Counter)) (result i32) (%effects reads writes)
        (%let $n (i32.add (struct.get $Counter $n (local.get $c)) (i32.const 1))
          (block (result i32)
            (struct.set $Counter $n (local.get $c) (local.get $n))
            (local.get $n))))
      (func $main (export "main") (result i32) (%effects reads writes allocates)
        (%let $c (struct.new $Counter (i32.const 0))
          (%let $v (i32x4.sub
                    (i32x4.splat (call $tick (local.get $c)))
                    (i32x4.splat (call $tick (local.get $c))))
            (i32.add (i32.mul (struct.get $Counter $n (local.get $c)) (i32.const 10))
              (i32x4.extract_lane 0 (local.get $v)))))))"#;
    let compiled = run(source, "19");
    let main = compiled.analysis.functions.last().unwrap();
    assert!(main.semantic_effects.reads && main.semantic_effects.writes);
    assert!(
        wlt::check(&source.replace("(%effects reads writes allocates)", "(%effects pure)"))
            .is_err()
    );
}

#[test]
fn native_simd_memory_keeps_the_wat_path_without_bypassing_contracts() {
    let source = r#"(module
      (memory 1)
      (func (export "main") (result i32)
        (v128.store (i32.const 0) (i32x4.splat (i32.const 42)))
        (%let $v v128 (v128.load (i32.const 0))
          (i32x4.extract_lane 0 (local.get $v)))))"#;
    let compiled = wlt::compile(source).unwrap();
    assert!(compiled.analysis.opaque);
    assert!(compiled.ir.is_none());
    assert!(
        wlt::compile(&source.replace(
            "(result i32)\n        (v128.store",
            "(result i32) (%effects pure)\n        (v128.store"
        ))
        .is_err()
    );
}

#[test]
fn trusted_host_policies_also_reject_vector_abi_shapes() {
    for (ty, shape) in [("v128", "v128"), ("(%inst $list v128)", "(list v128)")] {
        let source = format!(
            r#"(module
          (%import "wlt:std" (type $list))
          (import "simd:host/math" "take" (func $take (param $v {ty})))
          (func $main (export "main") (result i32) (%effects pure) (i32.const 42)))"#
        );
        let policy = wlt::bindings::BindingPolicy::parse(
            &serde_json::json!({
                "imports": [{ "module": "simd:host/math", "name": "take", "params": [shape],
                    "result": "unit", "suspends": false, "capability": "math" }]
            })
            .to_string(),
        )
        .unwrap();
        for error in [
            wlt::check_with_policy(&source, &policy).err().unwrap(),
            wlt::component::compile_with_policy(&source, &policy)
                .err()
                .unwrap(),
            wlt::component::core_js::compile_with_policy(&source, &policy)
                .err()
                .unwrap(),
        ] {
            assert!(error.to_string().contains("v128"), "{error}");
        }
    }
}

#[test]
fn invalid_lanes_shuffle_constants_and_operand_types_are_rejected() {
    for expression in [
        "(v128.const i32x4 1 2 3)",
        "(v128.const bogus 1 2)",
        "(i32x4.extract_lane 4 (i32x4.splat (i32.const 1)))",
        "(i8x16.replace_lane -1 (i8x16.splat (i32.const 1)) (i32.const 0))",
        "(i8x16.shuffle 32 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 (i32x4.splat (i32.const 1)) (i32x4.splat (i32.const 2)))",
        "(i32x4.add (i32.const 1) (i32x4.splat (i32.const 2)))",
        "(i64x2.splat (i32.const 1))",
        "(i32x4.shl (i32x4.splat (i32.const 1)) (i64.const 1))",
        "(v128.bitselect (i32x4.splat (i32.const 1)) (i32x4.splat (i32.const 2)))",
    ] {
        let source = format!("(module (func $bad (%effects pure) (drop {expression})))");
        assert!(wlt::check(&source).is_err(), "accepted: {source}");
        assert!(
            wlt::compile(&source).is_err(),
            "opaque fallback accepted invalid SIMD: {source}"
        );
    }
}

#[test]
fn portable_boundaries_reject_vectors_recursively_and_laws_do_not_skip_them() {
    for ty in [
        "v128",
        "(%inst $list v128)",
        "(%inst $option v128)",
        "(%inst $pair i32 v128)",
        "(%inst $future v128)",
    ] {
        let source = format!(
            r#"(module (%import "wlt:std" (type $list) (type $option) (type $pair) (type $future)) (func $f (export "f") (param $x {ty}) (result {ty}) (%effects pure) (local.get $x)))"#
        );
        // Future is owned; the corresponding move below keeps the boundary test valid.
        let source = if ty.starts_with("(%inst $future") {
            source.replace("(local.get $x)", "(%move $x)")
        } else {
            source
        };
        wlt::check(&source).unwrap();
        for error in [
            wlt::wit::emit(&wlt::check(&source).unwrap()).err().unwrap(),
            wlt::component::compile(&source).err().unwrap(),
            wlt::component::core_js::compile(&source).err().unwrap(),
        ] {
            assert!(error.to_string().contains("v128"), "{error}");
        }
    }
    assert!(
        wlt::compile("(module (%law \"SIMD\" (v128.any_true (v128.const i32x4 1 0 0 0))))")
            .is_err()
    );
}
