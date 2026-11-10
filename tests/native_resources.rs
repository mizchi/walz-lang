const SOURCE: &str = include_str!("component-fixtures/native-resources.walz");
fn policy() -> walz::bindings::BindingPolicy {
    walz::bindings::BindingPolicy::parse(include_str!("component-fixtures/native-resources.json"))
        .unwrap()
}
#[test]
fn native_resource_own_and_borrow_use_the_original_wit_identity() {
    let wit = std::path::Path::new("tests/component-fixtures/native-resources.wit");
    let c = walz::component::compile_with_world(SOURCE, &policy(), wit, None).unwrap();
    assert!(c.wat.contains("canon resource.drop"));
    for source in [
        SOURCE.replace("(@move $f)", "$f"),
        SOURCE.replace("(call $read $f)", "(call $read (i32.const 1))"),
        SOURCE.replace("test:resources/host/file", "other"),
    ] {
        assert!(
            walz::component::compile_with_world(&source, &policy(), wit, None).is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn generated_resource_bindings_check_own_borrow_and_keep_the_destructor() {
    let wit = std::path::Path::new("tests/component-fixtures/native-resources.wit");
    let generated=walz::wit_bindgen::generate(wit,None,r#"{"imports":[{"module":"test:resources/host","name":"make","suspends":false,"capability":"files"},{"module":"test:resources/host","name":"read","suspends":false,"capability":"files"}]}"#).unwrap();
    let policy = walz::bindings::BindingPolicy::parse(&generated.policy).unwrap();
    let module = walz::check_with_policy(&generated.source, &policy).unwrap();
    assert!(
        module
            .types
            .iter()
            .any(|t| matches!(t.kind, walz::ir::TypeKind::Resource { .. }))
    );
    assert_eq!(policy.imports.len(), 3);
    assert!(generated.types_source.contains("[resource-drop]file"));
}

#[test]
fn generated_typed_imports_and_reexported_resource_aliases_compile() {
    let dir = tempfile::tempdir().unwrap();
    let wit = dir.path().join("api.wit");
    let aliased = r#"package test:resources;
      interface base {resource file;}
      interface host {use base.{file as item}; make: func() -> item; read: func(f: borrow<item>) -> s32;}
      interface api {use host.{item}; echo: func(f:item)->item; read:func(f:borrow<item>)->s32; close:func(f:item); make:func()->item;}
      world guest {import host;export api;}"#;
    for (text, resource) in [
        (
            include_str!("component-fixtures/native-resources.wit"),
            "$host.file",
        ),
        (aliased, "$base.file"),
    ] {
        let text = text
            .replace("  optional: func(f: file) -> option<file>;", "")
            .replace("  release-option: func(f: option<file>);", "");
        std::fs::write(&wit, &text).unwrap();
        let generated=walz::wit_bindgen::generate(&wit,None,r#"{"imports":[{"module":"test:resources/host","name":"make","suspends":false,"capability":"files"},{"module":"test:resources/host","name":"read","suspends":false,"capability":"files"}]}"#).unwrap();
        let api=r#"
          (func $echo (export "api-echo") (param $f (@resource @R)) (result (@resource @R)) (@effect pure) (@move $f))
          (func $read (export "api-read") (param $f (@borrow @R)) (result i32) (@effect host reads writes allocates) (call $host.read $f))
          (func $close (export "api-close") (param $f (@resource @R)) (@effect host reads writes allocates) (@resource.close (@move $f)))
          (func $make (export "api-make") (result (@resource @R)) (@effect host reads writes allocates) (call $host.make))
        )"#.replace("@R",resource);
        let source = generated
            .source
            .trim_end()
            .strip_suffix(')')
            .unwrap()
            .to_owned()
            + &api;
        let policy = walz::bindings::BindingPolicy::parse(&generated.policy).unwrap();
        walz::component::compile_with_world(&source, &policy, &wit, None).unwrap();
        assert!(
            walz::component::core_js::compile_with_policy(&source, &policy).is_err(),
            "typed resource value ABI requires a native WIT adapter"
        );
    }
}

#[test]
fn numeric_resource_factories_are_not_native_own_handles() {
    let source = r#"(module (import "test:raw/host" "make" (func $make (result i32))) (import "test:raw/host" "close" (func $close (param $id i32))) (func $f (export "run") (@effect host) (drop (call $make))))"#;
    let policy=walz::bindings::BindingPolicy::parse(r#"{"imports":[{"module":"test:raw/host","name":"make","params":[],"result":"i32","suspends":false,"capability":"raw","resource":{"kind":"raw","operation":"new"}},{"module":"test:raw/host","name":"close","params":["i32"],"result":"unit","suspends":false,"capability":"raw","resource":{"kind":"raw","operation":"close"}}]}"#).unwrap();
    walz::check_with_policy(source, &policy).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let wit = dir.path().join("raw.wit");
    std::fs::write(&wit,"package test:raw; interface host {make:func()->s32;close:func(id:s32);} world guest {import host;export run:func();}").unwrap();
    let error = walz::component::compile_with_world(source, &policy, &wit, None)
        .err()
        .unwrap();
    assert_eq!(error.diagnostic().code, "E_WIT_WORLD");
    assert!(error.to_string().contains("typed own/borrow"));
}
