use std::process::Command;

fn valid(bytes: &[u8]) {
    assert!(wasmparser::Parser::is_component(bytes));
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .unwrap();
}

#[test]
fn exposes_internal_generic_functions_and_emits_the_actual_wit() {
    let source = r#"(component
      (@world "example:calc@0.1.0" "app")
      (@lib
        (func $add (param $a i32) (param $b i32) (result i32) (@effect pure) (+ $a $b))
        (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x))
      (@expose "add" $add)
      (@expose "identity" $id<i64>))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(
        built.wit.contains("package example:calc@0.1.0"),
        "{}",
        built.wit
    );
    assert!(built.wit.contains("world app"));
    assert!(
        built.wit.contains("add: func(a: s32, b: s32) -> s32"),
        "{}",
        built.wit
    );
    assert!(built.wit.contains("identity: func(x: s64) -> s64"));
    assert!(!built.wit.contains("walz:runtime"));
    assert_eq!(built.interface.exports.len(), 2);
}

#[test]
fn component_source_loads_lib_files_and_build_generates_world_wit() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("math.walz"),
        r#"(@lib (@export (func $answer)) (func $answer (result i32) (@effect pure) 42))"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("main.walz"), r#"(component (@world "example:calc@0.1.0" "app") (@use "./math.walz" (func $answer)) (@expose "answer" $answer))"#).unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(dir.path())
        .args(["build", "--wit"])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    valid(&std::fs::read(dir.path().join("target/main.component.wasm")).unwrap());
    let wit = std::fs::read_to_string(dir.path().join("target/world.wit")).unwrap();
    assert!(wit.contains("answer: func() -> s32"));
    assert!(walz::check(&std::fs::read_to_string(dir.path().join("math.walz")).unwrap()).is_ok());
    assert_eq!(
        walz::compile("(@lib)")
            .err()
            .expect("must reject")
            .diagnostic()
            .code,
        "E_LIBRARY_ENTRY"
    );
}

#[test]
fn binds_component_functions_to_internal_import_ports() {
    let source = r#"(component
      (core module $Host (func (export "double") (param i32) (result i32) (i32.mul (local.get 0) (i32.const 2))))
      (core instance $host (instantiate $Host))
      (func $double (param "x" s32) (result s32) (canon lift (core func $host "double")))
      (@lib
        (import "host" "double" (func $double (param $x i32) (result i32)))
        (func $answer (result i32) (@effect host) (call $double 21)))
      (@bind "host" "double" $double)
      (@expose "answer" $answer))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(built.interface.imports.is_empty());
    assert!(!built.wit.contains("import"));
}

#[test]
fn preserves_standard_components_and_explicit_instantiation() {
    let source = r#"(component
      (core module $M
        (memory (export "data") 1) (memory (export "abi") 1)
        (global $counter (mut i32) (i32.const 0))
        (func (export "next") (result i32) (global.set $counter (i32.add (global.get $counter) (i32.const 1))) (global.get $counter)))
      (core instance $a (instantiate $M))
      (core instance $b (instantiate $M))
      (alias core export $a "abi" (core memory $abi))
      (func (export "next-a") (result s32) (canon lift (core func $a "next") (memory $abi)))
      (func (export "next-b") (result s32) (canon lift (core func $b "next"))))"#;
    let built = walz::compile(source).unwrap();
    valid(&built.wasm);
    assert_eq!(built.wasm, wat::parse_str(source).unwrap());
    let view = walz::composition::inspect(&built.wasm).unwrap();
    assert_eq!(view.memories.len(), 4);
    assert_ne!(view.memories[0].identity, view.memories[2].identity);
}

#[test]
fn rejects_boundary_errors_without_erasing_contracts() {
    for (source, code) in [
        ("(@lib (func $f (export \"f\")))", "E_LIBRARY_ABI"),
        (
            "(component (@lib (func $f (result i32) (@effect pure) (i64.const 1))) (@expose \"f\" $f))",
            "E_TYPE_MISMATCH",
        ),
        (
            "(component (@lib (type $S (struct)) (func $f (result (ref $S)) (@effect allocates) (struct.new $S))) (@expose \"f\" $f))",
            "E_ABI_TYPE",
        ),
        (
            "(component (@lib (func $f (result i32) (@effect pure) 1)) (@expose \"f\" $f) (@expose \"f\" $f))",
            "E_COMPONENT_SOURCE",
        ),
    ] {
        let error = walz::compile(source).err().expect("must reject");
        assert_eq!(error.diagnostic().code, code, "{error}");
    }
}

#[test]
fn external_component_use_loads_a_constructor_and_preserves_instances() {
    let dir = tempfile::tempdir().unwrap();
    let library = r#"(component (@custom "external-test" "opaque\00\ff")
      (core module $M (global $n (mut i32) (i32.const 0))
        (func (export "next") (result i32) (global.set $n (i32.add (global.get $n) (i32.const 1))) (global.get $n)))
      (core instance $m (instantiate $M))
      (func (export "next") (result s32) (canon lift (core func $m "next"))))"#;
    std::fs::write(
        dir.path().join("counter.wasm"),
        wat::parse_str(library).unwrap(),
    )
    .unwrap();
    let source = r#"(component
      (@use "./counter.wasm" (component $Counter))
      (instance $a (instantiate $Counter))
      (instance $b (instantiate $Counter))
      (alias export $a "next" (func $next))
      (@lib (import "counter" "next" (func $next (result i32)))
        (func $run (result i32) (@effect host) (call $next)))
      (@bind "counter" "next" $next) (@expose "run" $run)
      (export "b" (instance $b)))"#;
    std::fs::write(dir.path().join("main.walz"), source).unwrap();
    let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
    let built = walz::composition::compile(loaded.text(), &Default::default()).unwrap();
    valid(&built.wasm);
    let dependency = wat::parse_str(library).unwrap();
    assert!(
        built
            .wasm
            .windows(dependency.len())
            .any(|bytes| bytes == dependency),
        "binary dependency/custom sections must remain exact"
    );
    assert!(built.interface.imports.is_empty());
    assert!(loaded.paths().any(|p| p.ends_with("counter.wasm")));
}

#[test]
fn native_core_module_exports_need_not_have_a_wit_representation() {
    let source = "(component (core module $M (func (export \"identity\") (param externref) (result externref) (local.get 0))) (export \"kernel\" (core module $M)))";
    valid(&walz::compile(source).unwrap().wasm);
    let metadata = r#"(component (component $C (@custom "walz:embedded-unit-v1" "opaque")) (instance (instantiate $C)))"#;
    assert_eq!(
        walz::compile(metadata).unwrap().wasm,
        wat::parse_str(metadata).unwrap(),
        "unrelated custom sections must never execute binary payloads"
    );
}

#[test]
fn memory_imports_share_identity_and_canonical_options_select_the_right_memory() {
    let source = r#"(component
      (core module $Owner (memory (export "heap") 1) (memory (export "abi") 2))
      (core instance $owner (instantiate $Owner))
      (core module $Borrower (import "env" "abi" (memory 2))
        (export "memory" (memory 0)) (func (export "f") (result i32) (i32.const 42)))
      (core instance $user (instantiate $Borrower (with "env" (instance $owner))))
      (func (export "answer") (result s32) (canon lift (core func $user "f") (memory (core memory $user "memory")))))"#;
    let built = walz::compile(source).unwrap();
    let view = walz::composition::inspect(&built.wasm).unwrap();
    assert_eq!(view.memories.len(), 3);
    assert_eq!(view.memories[1].identity, view.memories[2].identity);
    assert_ne!(view.memories[0].identity, view.memories[2].identity);
    assert_eq!(
        view.canonical_memories[0].memory,
        Some(view.memories[1].identity)
    );
    assert!(view.unresolved_memories.is_empty());
}

#[test]
fn extended_component_constructors_work_without_outer_expose() {
    let source = r#"(component
      (component $Child (@lib (func $f (result i32) (@effect pure) 42)) (@expose "answer" $f))
      (instance $child (instantiate $Child)) (export "api" (instance $child)))"#;
    let built = walz::compile(source).unwrap();
    valid(&built.wasm);
    assert!(
        walz::composition::inspect(&built.wasm)
            .unwrap()
            .interface
            .wit
            .contains("answer: func() -> s32")
    );
}

#[test]
fn cli_component_inspection_and_wit_input_validate_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.walz"),
        "(component (@lib (func $f (result i32) (@effect pure) 42)) (@expose \"answer\" $f))",
    )
    .unwrap();
    for args in [
        vec!["check", "--json", "main.walz"],
        vec!["types", "main.walz"],
        vec!["wit", "main.walz"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_walz"))
            .current_dir(dir.path())
            .args(&args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    for (result, ok) in [("s32", true), ("string", false)] {
        std::fs::write(
            dir.path().join("expected.wit"),
            format!("package test:contract; world app {{ export answer: func() -> {result}; }}"),
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_walz"))
            .current_dir(dir.path())
            .args(["build", "--wit-input", "expected.wit", "-o", "output.wasm"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            ok,
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        if ok {
            std::fs::remove_file(dir.path().join("output.wasm")).unwrap();
        } else {
            assert!(!dir.path().join("output.wasm").exists());
        }
    }
    std::fs::write(
        dir.path().join("main.walz"),
        "(component (core module $M) (export \"kernel\" (core module $M)))",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(dir.path())
        .args(["build", "--wit", "-o", "output.wasm"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!dir.path().join("output.wasm").exists());
}

#[test]
fn bind_type_mismatch_and_false_purity_are_rejected() {
    let base = r#"(component (core module $M (func (export "f") (param i32) (result i32) (local.get 0)))
      (core instance $m (instantiate $M))
      (func $f (param "x" s32) (result s32) (canon lift (core func $m "f")))
      (@lib (import "dep" "f" (func $f (param $x TYPE) (result TYPE)))
        (func $run (param $x TYPE) (result TYPE) (@effect EFFECT) (call $f $x)))
      (@bind "dep" "f" $f) (@expose "run" $run))"#;
    for (ty, effect) in [("i64", "host"), ("i32", "pure")] {
        assert!(walz::compile(&base.replace("TYPE", ty).replace("EFFECT", effect)).is_err());
    }
}

#[test]
fn ffi_metadata_describes_only_real_core_signatures() {
    let source = r#"(component (core module $M
      (@custom "walz:ffi-v1" "{\"version\":1,\"functions\":[{\"export\":\"identity\",\"params\":[\"externref\"],\"results\":[\"externref\"]}]}")
      (func (export "identity") (param externref) (result externref) (local.get 0)))
      (export "kernel" (core module $M)))"#;
    valid(&walz::compile(source).unwrap().wasm);
    for invalid in [
        source.replace("\"identity\\\",", "\"missing\\\","),
        source.replace("params\\\":[\\\"externref", "params\\\":[\\\"i32"),
    ] {
        let e = walz::compile(&invalid).err().expect("must check metadata");
        assert_eq!(e.diagnostic().code, "E_FFI_METADATA", "{e}");
    }
}

#[test]
fn inferred_future_stream_and_value_boundaries_use_native_producers() {
    let source = r#"(component (@world "test:values@0.1.0" "app")
      (@lib (@use "walz:core" (type $future) (type $stream) (type $string) (type $bytes))
        (func $consume (param $f $future<i32>) (result i32) (@effect suspends) (@future.await i32 (@move $f)))
        (func $ready (result $future<i32>) (@effect allocates) (@future.ready i32 42))
        (func $prepend (param $s $stream<i32>) (result $stream<i32>) (@effect allocates) (@stream.cons i32 7 (@move $s)))
        (func $text (param $v $string) (result $future<$string>) (@effect allocates) (@future.ready $string $v))
        (func $echo (param $v $bytes) (result $bytes) (@effect pure) $v))
      (@expose "consume" $consume) (@expose "ready" $ready) (@expose "prepend" $prepend)
      (@expose "ready-text" $text) (@expose "echo" $echo))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(built.wit.contains("future<string>"));
    assert!(built.wit.contains("stream<s32>"));
    assert!(built.wit.contains("consume: async func"), "{}", built.wit);
    assert!(built.interface.imports.is_empty());
    assert!(built.wat.contains("canon task.return"));
}

#[test]
fn library_checks_are_not_builds_and_component_constructors_are_not_core_imports() {
    let policy=walz::bindings::BindingPolicy::parse(r#"{"imports":[{"module":"example:host/api","name":"f","params":[],"result":"i32","suspends":false,"capability":"call"}]}"#).unwrap();
    let lib = r#"(@lib (import "example:host/api" "f" (func $f (result i32)))
      (func $run (result i32) (@effect host) (call $f)))"#;
    walz::validate_with_policy(lib, &policy).unwrap();
    assert_eq!(
        walz::compile_with_policy(lib, &policy)
            .err()
            .expect("not an entry")
            .diagnostic()
            .code,
        "E_LIBRARY_ENTRY"
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.walz");
    std::fs::write(
        &path,
        "(module (@use \"./counter.wasm\" (component $Counter)))",
    )
    .unwrap();
    assert_eq!(
        walz::source::load(&path)
            .err()
            .expect("Component selection in a Core module")
            .diagnostic()
            .code,
        "E_COMPONENT_SOURCE"
    );
    for value in ["invalid", r#"{\"package\":\"bad\",\"world\":\"app\"}"#] {
        let raw = format!(
            r#"(component (@custom "walz:world-v1" "{value}") (core module $M) (export "kernel" (core module $M)))"#
        );
        assert!(
            walz::compile(&raw).is_err(),
            "reserved metadata cannot be hidden by a non-WIT API"
        );
    }
}

#[test]
fn async_binding_inference_keeps_suspension_and_ownership_checks() {
    let source = include_str!("component-fixtures/composition-async.walz");
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    assert!(built.wit.contains("run: async func"));
    assert_eq!(built.manifest["asyncExports"], serde_json::json!(["run"]));
    assert!(walz::compile(&source.replace("(@effect host suspends)", "(@effect host)")).is_err());
    assert!(walz::compile(&source.replace("(call $consume (@move $value))","(block (result i32) (drop (call $consume (@move $value))) (call $consume (@move $value)))")).is_err());
}
