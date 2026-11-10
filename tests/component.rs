use std::process::Command;

#[test]
fn component_build_emits_a_valid_canonical_interface_and_rejects_unknown_bindings() {
    let source = r#"(module
      (import "walz:host/event-loop@0.1.0" "sleep" (func $sleep (param $ms i32)))
      (func $main (export "main") (result (@option (@result i64 i32))) (@effect host suspends)
        (block (result (@option (@result i64 i32))) (call $sleep (i32.const 1))
          (@option.some (@result i64 i32) (@result.ok i64 i32 (i64.const 42))))))"#;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.walz");
    let output = dir.path().join("component.wasm");
    std::fs::write(&input, source).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("component")
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let wasm = std::fs::read(&output).unwrap();
    assert!(!wasmparser::Parser::is_core_wasm(&wasm));
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .unwrap();
    let wit = Command::new("wasm-tools")
        .args(["component", "wit"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(wit.status.success());
    let wit = String::from_utf8(wit.stdout).unwrap();
    assert!(
        wit.contains("main: async func() -> t1")
            && wit.contains("type t0 = result<s64, s32>")
            && wit.contains("type t1 = option<t0>"),
        "{wit}"
    );
    std::fs::write(
        &input,
        "(module (import \"unknown\" \"f\" (func $f)) (func $main (export \"main\") (call $f)))",
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("component")
        .arg(&input)
        .arg(dir.path().join("bad.wasm"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("trusted binding"));
}

#[test]
fn trusted_event_loop_contracts_are_fixed_and_ownership_still_applies() {
    for (source, message) in [
        (
            "(module (import \"walz:host/event-loop@0.1.0\" \"sleep\" (func $sleep (param $ms i64))))",
            "binding signature",
        ),
        (
            "(module (import \"walz:host/event-loop@0.1.0\" \"magic\" (func $magic)))",
            "unknown event-loop binding",
        ),
        (
            "(module (import \"walz:host/event-loop@0.1.0\" \"defer-i64\" (func $defer (param $ms i32) (param $n i64) (result (@future i64)))) (func $bad (@let $f (call $defer (i32.const 1) (i64.const 42)) (nop))))",
            "unconsumed owned",
        ),
    ] {
        let error = walz::check(source).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn imported_and_exported_interface_aliases_roundtrip_as_wit() {
    let buffers = walz::source::load("tests/component-fixtures/buffers.walz").unwrap();
    for source in [
        include_str!("component-fixtures/values.walz"),
        buffers.text(),
        include_str!("component-fixtures/event-loop.walz"),
        // Generated type names must not collide with public function names.
        r#"(module
          (func $f (export "t0") (result (@option @unit)) (@effect pure) (@option.some @unit))
          (func $g (export "walz-unit") (@effect pure) (nop)))"#,
    ] {
        let component = walz::component::compile(source).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("component.wasm");
        let wit = dir.path().join("component.wit");
        let encoded = dir.path().join("types.wasm");
        std::fs::write(&binary, component.wasm).unwrap();
        let decoded = Command::new("wasm-tools")
            .args(["component", "wit"])
            .arg(&binary)
            .arg("-o")
            .arg(&wit)
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        let result = Command::new("wasm-tools")
            .args(["component", "wit"])
            .arg(&wit)
            .arg("--wasm")
            .arg("-o")
            .arg(&encoded)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&std::fs::read(encoded).unwrap())
            .unwrap();
    }
}
