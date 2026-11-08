use std::path::Path;

fn library(directory: &Path) {
    let wit = directory.join("math.wit");
    std::fs::write(&wit, "package test:binary; world math { export add: func(a: s32, b: s32) -> s32; export next: func() -> s32; export echo: func(value: string) -> string; }").unwrap();
    let source = r#"(module
      (%use "wlt:core" (type $string))
      (memory $state 1)
      (func $add (export "add") (param $a i32) (param $b i32) (result i32) (%effect pure) (+ $a $b))
      (func $next (export "next") (result i32) (%effect reads writes)
        (block (result i32) (i32.store $state 0 (+ (i32.load $state 0) 1)) (i32.load $state 0)))
      (func $echo (export "echo") (param $value $string) (result $string) (%effect pure) $value))"#;
    let compiled =
        wlt::component::compile_with_world(source, &Default::default(), &wit, Some("math"))
            .unwrap();
    std::fs::write(directory.join("math.wasm"), compiled.wasm).unwrap();
}

#[test]
fn typed_component_exports_are_selected_cached_and_embedded() {
    let dir = tempfile::tempdir().unwrap();
    library(dir.path());
    std::fs::write(
        dir.path().join("facade.wlt"),
        r#"(module (%use "./math.wasm" (func $next $forward)) (%export (func $forward)))"#,
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(&main, r#"(module
      (%use "./math.wasm" (func $add $sum) (func $next))
      (%use "./facade.wlt" (func $forward))
      (func $main (export "main") (result i32) (%effect host) (call $sum (call $next) (call $forward))))"#).unwrap();
    let source = wlt::source::load(&main).unwrap();
    assert!(source.paths().any(|p| p.ends_with("math.wasm")));
    let checked = wlt::check(source.text()).unwrap();
    assert_eq!(
        checked
            .functions
            .iter()
            .filter(|f| f.name.ends_with(".next"))
            .count(),
        1
    );
    let compiled = wlt::component::compile(source.text()).unwrap();
    assert_eq!(compiled.wat.matches("(component $binary").count(), 1);
    let foreign = std::fs::read(dir.path().join("math.wasm")).unwrap();
    assert!(compiled.wasm.windows(foreign.len()).any(|w| w == foreign));
    let roundtrip = wat::parse_str(&compiled.wat).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&roundtrip)
        .unwrap();
    assert!(
        !compiled.manifest["imports"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["module"]
                .as_str()
                .is_some_and(|s| s.starts_with("wlt:component/")))
    );
    let core = wlt::compile(source.text()).err().unwrap();
    assert_eq!(core.diagnostic().code, "E_COMPONENT_USE_BACKEND");
    let core = wlt::component::core_js::compile(source.text())
        .err()
        .unwrap();
    assert_eq!(core.diagnostic().code, "E_COMPONENT_USE_BACKEND");
    // Loading snapshots binary contents as well as text files.
    std::fs::remove_file(dir.path().join("math.wasm")).unwrap();
    wlt::component::compile(source.text()).unwrap();
}

#[test]
fn wasm_uses_keep_type_effect_privacy_and_dce_checks() {
    let dir = tempfile::tempdir().unwrap();
    library(dir.path());
    let main = dir.path().join("main.wlt");
    for (body, message) in [
        (
            r#"(func $main (export "main") (result i32) (%effect host) (call $add (i64.const 1) 2))"#,
            "type",
        ),
        (
            r#"(func $main (export "main") (result i32) (%effect pure) (call $add 1 2))"#,
            "effect",
        ),
        (
            r#"(func $main (export "main") (result i32) (%effect host) (call $next))"#,
            "unknown",
        ),
    ] {
        std::fs::write(
            &main,
            format!(r#"(module (%use "./math.wasm" (func $add)) {body})"#),
        )
        .unwrap();
        let source = wlt::source::load(&main).unwrap();
        let error = wlt::check(source.text()).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
    std::fs::write(&main, r#"(module (%use "./math.wasm" (func $add)) (func $main (export "main") (result i32) (%effect pure) 42))"#).unwrap();
    let source = wlt::source::load(&main).unwrap();
    let compiled = wlt::component::compile(source.text()).unwrap();
    assert!(!compiled.wat.contains("(component $binary"));
    wlt::compile(source.text()).unwrap();
}

#[test]
fn unsupported_async_and_owned_exports_are_rejected_only_when_selected() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/component-fixtures/wasm-use");
    let source = std::fs::read_to_string(fixture.join("library.wlt")).unwrap();
    let compiled = wlt::component::compile_with_world(
        &source,
        &Default::default(),
        &fixture.join("library.wit"),
        Some("library"),
    )
    .unwrap();
    std::fs::write(dir.path().join("library.wasm"), compiled.wasm).unwrap();
    let main = dir.path().join("main.wlt");
    for selector in ["$math.later", "$math.double", "$math.consume"] {
        std::fs::write(&main, format!("(module\n (%use \"./library.wasm\" (func $math.add $add))\n (%use \"./library.wasm\" (func {selector})))")).unwrap();
        let error = wlt::source::load(&main).err().unwrap();
        assert_eq!(error.diagnostic().code, "E_WASM_USE");
        assert!(error.to_string().contains("main.wlt:3:"), "{error}");
        assert!(
            error.to_string().contains("cancellation/lifetime"),
            "{error}"
        );
    }
}

#[test]
fn missing_exports_foreign_imports_and_binary_includes_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    library(dir.path());
    let main = dir.path().join("main.wlt");
    std::fs::write(&main, "(module\n (%use \"./math.wasm\" (func $absent)))").unwrap();
    let source = wlt::source::load(&main).unwrap();
    let error = source.diagnostic(wlt::check(source.text()).unwrap_err());
    assert!(error.to_string().contains("main.wlt:2:"), "{error}");
    std::fs::write(&main, "(module (%include \"./math.wasm\"))").unwrap();
    assert!(
        wlt::source::load(&main)
            .err()
            .unwrap()
            .to_string()
            .contains("require %use")
    );
    let bytes = wat::parse_str("(component (import \"host\" (func)))").unwrap();
    std::fs::write(dir.path().join("math.wasm"), bytes).unwrap();
    std::fs::write(&main, "(module (%use \"./math.wasm\" (func $host)))").unwrap();
    let error = wlt::source::load(&main).err().unwrap();
    assert!(error.to_string().contains("must be closed"), "{error}");
}

#[test]
fn invalid_and_untyped_wasm_report_the_import_site() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    for bytes in [
        vec![0, 1, 2],
        wat::parse_str("(module (func (export \"add\")))").unwrap(),
    ] {
        std::fs::write(dir.path().join("math.wasm"), bytes).unwrap();
        std::fs::write(&main, "(module\n  (%use \"./math.wasm\" (func $add)))").unwrap();
        let error = wlt::source::load(&main).err().unwrap();
        assert_eq!(error.diagnostic().code, "E_WASM_USE");
        assert!(error.to_string().contains("main.wlt:2:"), "{error}");
    }
}
