#[test]
fn json_parser_is_a_checked_library_without_new_language_primitives() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/component-fixtures/json-value.wlt");
    let loaded = wlt::source::load(&path).unwrap();
    wlt::check(loaded.text()).unwrap();
    wlt::component::compile(loaded.text()).unwrap();
    wlt::component::core_js::compile(loaded.text()).unwrap();
}

#[test]
fn published_json_values_remain_immutable_for_effect_analysis() {
    wlt::compile(
        r#"(module
          (%import "wlt:std" (type $json-value.Value $Value) (type $option)
            (func $json-value.get) (func $json-value.at))
          (func $get (param $value (ref $Value))
            (result (%inst $option (ref $Value))) (%effect pure)
            (call $json-value.get (local.get $value) (%text.lit "key")))
          (func $at (param $value (ref $Value))
            (result (%inst $option (ref $Value))) (%effect pure)
            (call $json-value.at (local.get $value) (i32.const 0))))"#,
    )
    .unwrap();
}

#[test]
fn parser_boundaries_and_unicode_execute_in_native_wasm() {
    use std::process::Command;
    let encode = |s: &str| {
        s.as_bytes()
            .iter()
            .map(|b| format!("\\{b:02x}"))
            .collect::<String>()
    };
    let mut tests = Vec::new();
    for (input, accepted) in [
        ("[".repeat(128) + "0" + &"]".repeat(128), true),
        ("[".repeat(128) + &"]".repeat(128), true),
        ("[".repeat(129) + &"]".repeat(129), false),
        ("{\"x\":".repeat(128) + "{}" + &"}".repeat(128), false),
        (" ".repeat(65532) + "null", true),
        (" ".repeat(65533) + "null", false),
        ("\"\\uDBFF\\uDFFF\"".into(), true),
        ("\"\\uD800\"".into(), false),
    ] {
        tests.push(format!(
            r#"(func $case{} (export "case{}") (result i32)
          (%match (call $json-value.parse (%text.lit "{}"))
            (case ok ($value) (i32.const {}))
            (case err ($offset) (i32.const {}))))"#,
            tests.len(),
            tests.len(),
            encode(&input),
            u8::from(accepted),
            u8::from(!accepted)
        ));
    }
    let source = format!(
        "(module (%import \"wlt:std\" (func $json-value.parse)) {})",
        tests.join("\n")
    );
    let compiled = wlt::compile(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("json.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    for i in 0..tests.len() {
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                &format!("case{i}"),
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "1",
            "case {i}"
        );
    }
}
