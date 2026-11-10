use std::process::Command;

#[test]
fn standard_numeric_instructions_remain_in_the_typed_core() {
    for (result, expression, expected) in [
        ("i32", "(i32.trunc_f64_s (f64.sqrt (f64.const 1764)))", "42"),
        (
            "i32",
            "(i32.trunc_f32_s (f32.nearest (f32.const 41.5)))",
            "42",
        ),
        (
            "i64",
            "(i64.trunc_f64_u (f64.convert_i32_u (i32.const -1)))",
            "4294967295",
        ),
        ("i32", "(i32.trunc_sat_f64_s (f64.const nan))", "0"),
        ("i64", "(i64.trunc_sat_f32_u (f32.const inf))", "-1"),
        (
            "i32",
            "(i32.reinterpret_f32 (f32.copysign (f32.const 0) (f32.const -1)))",
            "-2147483648",
        ),
        (
            "i64",
            "(i64.reinterpret_f64 (f64.reinterpret_i64 (i64.const 42)))",
            "42",
        ),
        (
            "i32",
            "(i32.trunc_f32_s (f32.demote_f64 (f64.promote_f32 (f32.const 42))))",
            "42",
        ),
        ("i32", "(i32.rotr (i32.const 84) (i32.const 1))", "42"),
        ("i64", "(i64.extend8_s (i64.const 255))", "-1"),
        (
            "i32",
            "(i32.trunc_f64_s (f64.max (f64.floor (f64.const 42.9)) (f64.ceil (f64.const 40.1))))",
            "42",
        ),
    ] {
        let source = format!(
            "(module (func $main (export \"main\") (result {result}) (@effect pure) {expression}))"
        );
        let compiled = walz::compile(&source).unwrap();
        assert!(!compiled.analysis.opaque, "{expression}");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scalar.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let output = Command::new("wasmtime")
            .args(["run", "--invoke", "main"])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn numeric_conversion_types_and_trapping_effects_are_checked() {
    assert!(
        walz::check(
            "(module (func $bad (result f64) (@effect pure) (f64.convert_i64_s (i32.const 1))))"
        )
        .is_err()
    );
    for (op, trapping) in [("i32.trunc_f64_s", true), ("i32.trunc_sat_f64_s", false)] {
        let module = walz::check(&format!(
            "(module (func $f (param $x f64) (result i32) (@effect pure) ({op} (local.get $x))))"
        ))
        .unwrap();
        assert_eq!(
            walz::analysis::analyze(&module).functions[0]
                .effects
                .may_trap,
            trapping
        );
    }
}
