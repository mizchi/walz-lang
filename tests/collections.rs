use std::process::Command;

#[test]
fn dictionary_and_list_builder_contracts_run_on_wasm_gc() {
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "tests/collections.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("6 passed; 0 failed; 0 errors"));
}

#[test]
fn collection_bounds_effects_and_opaque_layouts_are_checked_before_dce() {
    for (source, expected) in [
        (
            r#"(module (%use "wlt:core" (func $dict.new) (type $future)) (func $bad (drop (call (%inst $dict.new (%inst $future i64))))))"#,
            "shared",
        ),
        (
            r#"(module (%use "wlt:core" (func $list-builder.new) (type $unit)) (func $bad (drop (call (%inst $list-builder.new $unit)))))"#,
            "nonunit",
        ),
        (
            r#"(module (%use "wlt:core" (type $dict) (type $string) (type $option) (func $dict.get)) (func $bad (param $d (ref (%inst $dict i64))) (param $key $string) (result (%inst $option i64)) (%effect pure) (call (%inst $dict.get i64) (local.get $d) (local.get $key))))"#,
            "effect contract",
        ),
        (
            r#"(module (%use "wlt:core" (type $dict)) (func $bad (param $d (ref (%inst $dict i64))) (result i32) (struct.get (%inst $dict i64) $size (local.get $d))))"#,
            "opaque",
        ),
        (
            r#"(module (%use "wlt:core" (type $list-builder)) (func $bad (param $b (ref (%inst $list-builder i64))) (result i32) (struct.get (%inst $list-builder i64) $length (local.get $b))))"#,
            "opaque",
        ),
    ] {
        let error = wlt::check(source).unwrap_err().to_string();
        assert!(error.contains(expected), "expected {expected}: {error}");
    }
}
