use std::path::Path;

fn library(directory: &Path) {
    let wit = directory.join("math.wit");
    std::fs::write(&wit, "package test:binary; world math { export add: func(a: s32, b: s32) -> s32; export next: func() -> s32; export echo: func(value: string) -> string; }").unwrap();
    let source = r#"(module
      (@use "walz:core" (type $string))
      (memory $state 1)
      (func $add (export "add") (param $a i32) (param $b i32) (result i32) (@effect pure) (+ $a $b))
      (func $next (export "next") (result i32) (@effect reads writes)
        (block (result i32) (i32.store $state 0 (+ (i32.load $state 0) 1)) (i32.load $state 0)))
      (func $echo (export "echo") (param $value $string) (result $string) (@effect pure) $value))"#;
    let compiled =
        walz::component::compile_with_world(source, &Default::default(), &wit, Some("math"))
            .unwrap();
    std::fs::write(directory.join("math.wasm"), compiled.wasm).unwrap();
}

fn entry(directory: &Path, body: &str) -> walz::source::Source {
    let main = directory.join("main.walz");
    std::fs::write(&main, body).unwrap();
    walz::source::load(&main).unwrap()
}

#[test]
fn constructors_snapshot_original_binary_and_connect_selected_functions() {
    let dir = tempfile::tempdir().unwrap();
    library(dir.path());
    std::fs::write(
        dir.path().join("facade.walz"),
        r#"(@lib
      (import "math" "next" (func $next (result i32)))
      (func $forward (result i32) (@effect host) (call $next))
      (@export (func $forward)))"#,
    )
    .unwrap();
    let source = entry(
        dir.path(),
        r#"(component
      (@use "./math.wasm" (component $Math))
      (instance $math (instantiate $Math))
      (alias export $math "add" (func $sum))
      (alias export $math "next" (func $next))
      (@lib (@use "./facade.walz" (func $forward))
        (import "math" "add" (func $add (param $a i32) (param $b i32) (result i32)))
        (func $main (result i32) (@effect host) (call $add (call $forward) (call $forward))))
      (@connect "math" "add" $sum) (@connect "math" "next" $next)
      (@expose "main" $main))"#,
    );
    assert!(source.paths().any(|p| p.ends_with("math.wasm")));
    let foreign = std::fs::read(dir.path().join("math.wasm")).unwrap();
    std::fs::remove_file(dir.path().join("math.wasm")).unwrap();
    let built = walz::composition::compile(source.text(), &Default::default()).unwrap();
    assert!(built.wasm.windows(foreign.len()).any(|w| w == foreign));
    assert!(built.manifest["imports"].as_array().unwrap().is_empty());
    assert_eq!(built.manifest["connections"].as_array().unwrap().len(), 2);
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wat::parse_str(&built.wat).unwrap())
        .unwrap();
}

#[test]
fn explicit_connections_keep_type_effect_and_capability_checks() {
    let dir = tempfile::tempdir().unwrap();
    library(dir.path());
    for (body, needle) in [
        ("(call $add (i64.const 1) 2)", "type"),
        ("(call $hidden)", "unknown"),
        ("(call $add 1 2)", "effect"),
    ] {
        let effect = if needle == "effect" { "pure" } else { "host" };
        let source = entry(
            dir.path(),
            &format!(
                r#"(component
          (@use "./math.wasm" (component $Math)) (instance $m (instantiate $Math))
          (alias export $m "add" (func $foreign))
          (@lib (import "math" "add" (func $add (param $a i32) (param $b i32) (result i32)))
            (func $run (result i32) (@effect {effect}) {body}))
          (@connect "math" "add" $foreign) (@expose "run" $run))"#
            ),
        );
        let error = walz::compile(source.text()).err().unwrap();
        assert!(error.to_string().contains(needle), "{error}");
    }
}

#[test]
fn binary_constructors_can_require_explicit_imports() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = wat::parse_str(
        r#"(component (import "host" (func $host (result s32))) (export "answer" (func $host)))"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("math.wasm"), bytes).unwrap();
    let source = entry(
        dir.path(),
        r#"(component
      (import "host" (func $host (result s32)))
      (@use "./math.wasm" (component $Math))
      (instance $m (instantiate $Math (with "host" (func $host))))
      (alias export $m "answer" (func $answer)) (export "answer" (func $answer)))"#,
    );
    let built = walz::compile(source.text()).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&built.wasm)
        .unwrap();
    let error = walz::compile(&source.text().replace(" (with \"host\" (func $host))", ""))
        .err()
        .unwrap();
    assert!(error.to_string().contains("host"), "{error}");
}

#[test]
fn binary_constructor_errors_keep_the_original_use_site() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.walz");
    for bytes in [
        vec![0, 1, 2],
        wat::parse_str("(module)").unwrap(),
        vec![0; 4 * 1024 * 1024 + 1],
    ] {
        std::fs::write(dir.path().join("math.wasm"), bytes).unwrap();
        std::fs::write(
            &main,
            "(component\n (@use \"./math.wasm\" (component $Math)))",
        )
        .unwrap();
        let error = walz::source::load(&main).err().unwrap();
        assert_eq!(error.diagnostic().code, "E_COMPONENT_SOURCE");
        assert!(error.to_string().contains("main.walz:2:"), "{error}");
    }
    std::fs::write(&main, "(component (@include \"./math.wasm\"))").unwrap();
    assert_eq!(
        walz::source::load(&main).err().unwrap().diagnostic().code,
        "E_WASM_USE"
    );
}

#[test]
fn async_and_owned_binary_ports_use_native_component_contracts() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/component-fixtures/wasm-use");
    for name in ["library.walz", "library.wit", "main.walz", "facade.walz"] {
        std::fs::copy(fixture.join(name), dir.path().join(name)).unwrap();
    }
    let source = walz::source::load(dir.path().join("library.walz")).unwrap();
    let binary = walz::component::compile_with_world(
        source.text(),
        &Default::default(),
        &dir.path().join("library.wit"),
        Some("library"),
    )
    .unwrap();
    std::fs::write(dir.path().join("library.wasm"), binary.wasm).unwrap();
    let source = walz::source::load(dir.path().join("main.walz")).unwrap();
    let built = walz::composition::compile(source.text(), &Default::default()).unwrap();
    assert!(built.wit.contains("future<s64>"), "{}", built.wit);
    assert!(built.wit.contains("async func"), "{}", built.wit);
}
