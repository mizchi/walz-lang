use std::process::Command;

#[test]
fn core_js_builds_valid_modules_and_only_required_boundary_codecs() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.walz");
    let output = dir.path().join("output");
    std::fs::write(
        &source,
        r#"(module (func $internal (result @string) (@effect pure) (@string.lit "internal"))
          (func $f (export "main") (param $x i64) (result i64)
          (@effect pure) (local.get $x)))"#,
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("core-js")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for name in ["core.wasm", "memory.wasm"] {
        let wasm = std::fs::read(output.join(name)).unwrap();
        assert!(wasmparser::Parser::is_core_wasm(&wasm));
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&wasm)
            .unwrap();
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["abi"], "walz-core-js-v2");
    let bindings = std::fs::read_to_string(output.join("bindings.mjs")).unwrap();
    assert!(bindings.contains("BigInt64"));
    assert!(!bindings.contains("Float64") && !bindings.contains("TextEncoder"));
}

#[test]
fn core_js_does_not_overwrite_its_source_with_an_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("core.wasm");
    let text = "(module (func $f (export \"main\") (@effect pure) (nop)))";
    std::fs::write(&source, text).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("core-js")
        .arg(&source)
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read_to_string(source).unwrap(), text);
}

#[test]
fn core_js_preserves_trusted_import_and_portable_export_checks() {
    for (source, message) in [
        (
            r#"(module (import "unknown" "f" (func $f))
             (func $main (export "main") (call $f)))"#,
            "trusted binding",
        ),
        (
            r#"(module (type $Box (struct (field i64)))
             (func $main (export "main") (result (ref $Box))
               (struct.new $Box (i64.const 42))))"#,
            "GC references",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.walz");
        std::fs::write(&input, source).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_walz"))
            .arg("core-js")
            .arg(input)
            .arg(dir.path().join("output"))
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
