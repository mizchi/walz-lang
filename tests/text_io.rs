#[test]
fn text_io_contracts_check_types_effects_and_capabilities_in_both_backends() {
    let source = include_str!("../examples/io/text-filter.wlt");
    for manifest in [
        wlt::component::compile(source).unwrap().manifest,
        wlt::component::core_js::compile(source).unwrap().manifest,
    ] {
        let imports = manifest["imports"].as_array().unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0]["module"], "wlt:host/text-io@0.1.0");
        assert_eq!(imports[0]["capability"], "text-input");
        assert_eq!(imports[1]["capability"], "text-output");
        assert_eq!(imports[1]["params"], serde_json::json!(["text"]));
        assert_eq!(imports[1]["effects"]["host"], true);
    }
    for (source, expected) in [
        (
            source.replace("(%effects host suspends)", "(%effects suspends)"),
            "effect",
        ),
        (
            r#"(module (import "wlt:host/text-io@0.1.0" "write-line" (func $write (param $line %bytes) (result (%future (%result %unit i32))))))"#.into(),
            "binding signature",
        ),
        (
            source.replace("\"read-lines\"", "\"read-anything\""),
            "unknown text-io binding",
        ),
    ] {
        let error = wlt::check(&source).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn clock_and_text_capabilities_are_distinct_component_interfaces() {
    let source = include_str!("../examples/io/text-filter.wlt").replacen(
        "(module",
        "(module (import \"wlt:host/event-loop@0.1.0\" \"yield\" (func $yield))",
        1,
    );
    let source = source.trim_end().strip_suffix(')').unwrap().to_owned()
        + "(func $clock (export \"clock\") (%effects host suspends) (call $yield)))";
    let compiled = wlt::component::compile(&source).unwrap();
    assert!(compiled.wat.contains("wlt:host/event-loop@0.1.0"));
    assert!(compiled.wat.contains("wlt:host/text-io@0.1.0"));
    assert_eq!(compiled.manifest["imports"].as_array().unwrap().len(), 3);
}

#[test]
fn text_io_wit_contract_is_valid_with_future_stream_and_result_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("text-io.wasm");
    let output = std::process::Command::new("wasm-tools")
        .args(["component", "wit", "interfaces/text-io.wit", "--wasm", "-o"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&std::fs::read(path).unwrap())
        .unwrap();
}
