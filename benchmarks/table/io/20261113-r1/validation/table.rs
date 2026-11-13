use std::process::Command;

#[test]
fn table_roots_keep_checked_ir_and_gc_storage() {
    for path in [
        "examples/table/main.walz",
        "examples/table/kernel.walz",
        "examples/table/portable.walz",
    ] {
        let source = walz::source::load(path).unwrap();
        walz::check(source.text()).unwrap();
    }
    let source = walz::source::load("examples/table/kernel.walz").unwrap();
    let compiled =
        walz::compile_with_options(source.text(), walz::CompileOptions { optimize: false })
            .unwrap();
    assert!(!compiled.analysis.opaque);
    assert!(compiled.ir.is_some());
    assert!(
        compiled.wat.contains("(memory "),
        "the raw facade has a bounded transfer buffer"
    );
    assert!(!compiled.wat.contains("(import "));
    assert!(
        compiled.wat.contains("f64x2.lt"),
        "numeric filters must emit SIMD comparisons"
    );
    assert!(
        compiled.wat.contains("i64x2.bitmask"),
        "SIMD lane masks must select rows"
    );
}

#[test]
fn table_storage_and_effect_contracts_are_checked() {
    for (body, expected) in [
        (
            "(func $bad (param $t (ref $table)) (drop (struct.get $table 0 $t)))",
            "opaque",
        ),
        (
            "(func $bad (param $c (ref $column)) (drop (struct.get $column 0 $c)))",
            "opaque",
        ),
        (
            "(func $bad (param $t (ref $table)) (param $e (ref $expr)) (result (ref $table)) (@effect pure) (call $table.filter $t $e))",
            "effect contract",
        ),
        (
            "(func $bad (param $c (ref $column)) (@effect pure) (drop (call $column.values $c)))",
            "effect contract",
        ),
    ] {
        let directory = tempfile::tempdir_in(".").unwrap();
        let path = directory.path().join("main.walz");
        std::fs::write(
            &path,
            format!(
                r#"(module
          (@use "../walz/table/engine.walz" (type $table) (type $expr) (type $column) (func $table.filter) (func $column.values))
          {body})"#
            ),
        )
        .unwrap();
        let source = walz::source::load(path).unwrap();
        let error = walz::check(source.text()).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn native_table_programs_execute_on_wasmtime() {
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .args(["test", "tests/table.test.walz"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("4 passed; 0 failed; 0 errors"));
}
