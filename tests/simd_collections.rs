use std::process::Command;

#[test]
fn simd_collections_use_checked_gc_storage_and_vector_instructions() {
    let source = walz::source::load("examples/simd-collections/portable.walz").unwrap();
    let compiled = walz::compile(source.text()).unwrap();
    assert!(!compiled.analysis.opaque);
    assert!(compiled.ir.is_some());
    for instruction in [
        "v128.and",
        "i8x16.popcnt",
        "i32x4.bitmask",
        "i64x2.extend_low_i32x4_s",
    ] {
        assert!(compiled.wat.contains(instruction), "{instruction}");
    }
    assert!(!compiled.wat.contains("(memory "));
    assert!(!compiled.wat.contains("(import "));
}

#[test]
fn simd_collection_storage_is_opaque_and_effects_remain_checked() {
    let prefix = r#"(module
      (@use "./walz/simd/bitmap.walz" (type $bitmap) (func $bitmap.count))"#;
    for (body, diagnostic) in [
        (
            "(func $bad (param $b (ref $bitmap)) (result i32) (struct.get $bitmap 0 $b))",
            "opaque",
        ),
        (
            "(func $bad (param $b (ref $bitmap)) (result i32) (@effect pure) (call $bitmap.count $b))",
            "effect contract",
        ),
    ] {
        let directory = tempfile::tempdir_in(".").unwrap();
        // Resolve imports from the repository without writing into its root.
        let source = format!("{prefix} {body})").replace("./walz/", "../walz/");
        let input = directory.path().join("main.walz");
        std::fs::write(&input, source).unwrap();
        let loaded = walz::source::load(&input).unwrap();
        let error = walz::check(loaded.text()).unwrap_err().to_string();
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn simd_collection_examples_execute_on_wasmtime() {
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .args(["test", "tests/simd-collections.test.walz"])
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
