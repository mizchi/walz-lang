use std::process::Command;

#[test]
fn dictionary_columns_stay_in_the_checked_core_and_reuse_simd() {
    for input in [
        "examples/dictionary-column/portable.walz",
        "examples/dictionary-column/bench-resident.walz",
        "examples/dictionary-column/fused.walz",
    ] {
        let loaded = walz::source::load(input).unwrap();
        let compiled = walz::compile(loaded.text()).unwrap();
        assert!(!compiled.analysis.opaque, "{input}");
        assert!(compiled.ir.is_some(), "{input}");
        assert!(compiled.wat.contains("i32x4.eq"), "{input}");
        assert!(compiled.wat.contains("i32x4.bitmask"), "{input}");
        assert!(!compiled.wat.contains("(memory "), "{input}");
        assert!(!compiled.wat.contains("(import "), "{input}");
    }
}

#[test]
fn dictionary_storage_and_read_effects_are_not_bypassed() {
    for (body, expected) in [
        (
            "(func $bad (param $c (ref $dictionary-column)) (drop (struct.get $dictionary-column 0 $c)))",
            "opaque",
        ),
        (
            "(func $bad (param $c (ref $dictionary-column)) (result i32) (@effect pure) (call $dictionary-column.count-eq $c \"JP\"))",
            "effect contract",
        ),
        (
            "(func $bad (param $c (ref $dictionary-column)) (param $v (ref $i32-column)) (result (ref $i32-query-result)) (@effect pure) (call $dictionary-column.query-between $c $v (@list.new $string \"JP\") 0 1))",
            "effect contract",
        ),
    ] {
        let directory = tempfile::tempdir_in(".").unwrap();
        let input = directory.path().join("main.walz");
        std::fs::write(&input, format!(r#"(module
          (@use "walz:core" (type $list) (type $string))
          (@use "../walz/simd/dictionary-column.walz" (type $dictionary-column) (func $dictionary-column.count-eq) (func $dictionary-column.query-between))
          (@use "../walz/simd/i32-column.walz" (type $i32-column) (type $i32-query-result))
          {body})"#)).unwrap();
        let loaded = walz::source::load(&input).unwrap();
        let error = walz::check(loaded.text()).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn dictionary_column_language_tests_execute_natively() {
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .args(["test", "tests/dictionary-column.test.walz"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("passed; 0 failed; 0 errors"));
}
