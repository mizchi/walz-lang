use walz::component::js_native::{Mode, compile};

#[test]
fn experimental_direct_abi_keeps_the_checked_gc_guest_and_does_not_emit_task_or_component_glue() {
    let source = walz::source::load("examples/flatbuffers/main.walz").unwrap();
    for mode in [Mode::Utf8, Mode::JsStrings] {
        let built = compile(source.text(), mode).unwrap();
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&built.wasm)
            .unwrap();
        assert!(!built.wat.contains("__component"));
        assert!(!built.wat.contains("__task_cancel"));
        assert!(!built.bindings.contains("Tasks"));
        assert!(!built.bindings.contains("Promise"));
        assert_eq!(built.manifest["jsStrings"], mode == Mode::JsStrings);
        assert_eq!(
            built.wat.contains("wasm:js-string"),
            mode == Mode::JsStrings
        );
    }
}

#[test]
fn unsupported_effects_owners_memories_and_boundary_shapes_are_explicit_errors() {
    for source in [
        "(module (@use \"walz:core\" (type $future)) (func $f (export \"f\") (result (@inst $future i32)) (@effect allocates) (@future.ready i32 1)))",
        "(module (memory $memory 1) (func $f (export \"f\") (result i32) (@effect reads) (i32.load $memory (i32.const 0))))",
    ] {
        walz::component::core_js::compile(source).unwrap();
        assert_eq!(
            compile(source, Mode::JsStrings)
                .err()
                .unwrap()
                .diagnostic()
                .code,
            "E_JS_NATIVE_UNSUPPORTED",
            "{source}"
        );
    }
    assert!(compile("(module (type $S (struct (field i32))) (func $f (export \"f\") (result (ref $S)) (@effect allocates) (struct.new $S 42)))", Mode::JsStrings).is_err());
    assert!(
        compile(
            "(module (func $f (export \"f\") (result i32) (@effect pure) (call $missing)))",
            Mode::JsStrings
        )
        .is_err()
    );
}

#[test]
fn scalar_only_modules_do_not_require_a_source_byte_type() {
    for mode in [Mode::Utf8, Mode::JsStrings] {
        compile(
            "(module (func $answer (export \"answer\") (result i32) (@effect pure) 42))",
            mode,
        )
        .unwrap();
    }
}
