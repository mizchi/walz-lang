#[test]
fn immediate_shared_constructor_matches_do_not_allocate_temporary_boxes() {
    for expression in [
        "(%match (%result i32 i32) (result i32) (%result.ok i32 i32 (i32.const 42)) (case ok ($n) (local.get $n)) (case err ($e) (local.get $e)))",
        "(%let ($a $b) (%pair.new i32 i32 (i32.const 40) (i32.const 2)) (i32.add (local.get $a) (local.get $b)))",
    ] {
        let source = format!("(module (func $main (export \"main\") (result i32) {expression}))");
        let baseline =
            wlt::compile_with_options(&source, wlt::CompileOptions { optimize: false }).unwrap();
        let optimized = wlt::compile(&source).unwrap();
        assert!(baseline.wat.contains("struct.new $__variant"));
        assert!(
            !optimized.wat.contains("struct.new $__variant"),
            "{}",
            optimized.wat
        );
        assert!(optimized.wasm.len() < baseline.wasm.len());
    }
}

#[test]
fn owned_payloads_keep_the_checked_match_protocol() {
    let source = "(module (func $main (export \"main\") (%let ($a $b) (%pair.new (%own i64) i32 (%own.new i64 (i64.const 42)) (i32.const 1)) (%own.close (%move $a)))))";
    assert!(wlt::compile(source).unwrap().wat.contains("ref.cast"));
}

#[test]
fn unused_payload_traps_are_preserved_and_raw_wat_is_unchanged() {
    let source = "(module (func $main (export \"main\") (result i32) (%let ($a $b) (%pair.new i32 i32 (i32.const 42) (i32.div_s (i32.const 1) (i32.const 0))) (local.get $a))))";
    for optimize in [false, true] {
        let built = wlt::compile_with_options(source, wlt::CompileOptions { optimize }).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("program.wasm");
        std::fs::write(&path, built.wasm).unwrap();
        let output = std::process::Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                "main",
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("integer divide by zero"));
    }
    let wat = "(module (func (export \"main\") (result i32) i32.const 42))";
    assert_eq!(wlt::compile(wat).unwrap().wat, wat);
}

#[test]
fn removing_an_unreachable_suspend_preserves_the_public_async_contract() {
    let source = include_str!("component-fixtures/async-abi.wlt");
    let core = wlt::component::core_js::compile(source).unwrap();
    assert!(core.bindings.contains("asyncExports.push(\"main\")"));
    assert!(core.manifest["imports"].as_array().unwrap().is_empty());
    let component = wlt::component::compile(source).unwrap();
    assert_eq!(
        component.manifest["asyncExports"],
        serde_json::json!(["main"])
    );
    assert!(component.manifest["imports"].as_array().unwrap().is_empty());
}
