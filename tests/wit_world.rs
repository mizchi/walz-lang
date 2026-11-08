use std::process::Command;
#[test]
fn native_world_exports_use_original_interfaces_without_private_delivery_imports() {
    let dir = tempfile::tempdir().unwrap();
    let wit = dir.path().join("world.wit");
    std::fs::write(&wit,"package test:adapter; interface api { record item { value: s32 } type unused = tuple<bool,char,s8>; echo: func(v: item) -> item; } world guest { export api; } world empty {}").unwrap();
    let source = r#"(module (type $Item (struct (field $value i32))) (%abi.type $Item (record "value")) (func $echo (export "api-echo") (param $v (ref $Item)) (result (ref $Item)) (%effect pure) $v))"#;
    let source = source.trim_end().strip_suffix(')').unwrap().to_owned()
        + r#"
      (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $n i32)))
      (%closure $Work (result i32) (%effect pure) 42)
      (func $unused (export "unused") (%effect host reads writes allocates suspends)
        (block (call $sleep 1) (%task.drop i32 (%task.spawn i32 $Work (%closure.new $Work)))))
    )"#;
    let input = dir.path().join("main.wlt");
    std::fs::write(&input, source).unwrap();
    let output = dir.path().join("world.wasm");
    let result = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args([
            "build",
            input.to_str().unwrap(),
            "--target",
            "component",
            "--wit",
            wit.to_str().unwrap(),
            "--world",
            "guest",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let wasm = std::fs::read(&output).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .unwrap();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut depth = 0;
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        match payload.unwrap() {
            wasmparser::Payload::ComponentSection { .. }
            | wasmparser::Payload::ModuleSection { .. } => depth += 1,
            wasmparser::Payload::End(_) => {
                depth -= 1;
            }
            wasmparser::Payload::ComponentImportSection(s) if depth == 0 => {
                for i in s {
                    imports.push(i.unwrap().name.name.to_owned());
                }
            }
            wasmparser::Payload::ComponentExportSection(s) if depth == 0 => {
                for e in s {
                    exports.push(e.unwrap().name.name.to_owned());
                }
            }
            _ => {}
        }
    }
    assert!(imports.is_empty(), "{imports:?}");
    assert_eq!(exports, ["test:adapter/api"]);
    let result = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args([
            "build",
            input.to_str().unwrap(),
            "--target",
            "component",
            "--wit",
            wit.to_str().unwrap(),
            "--world",
            "missing",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
}
