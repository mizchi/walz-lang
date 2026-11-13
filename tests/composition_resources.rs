const SOURCE: &str = include_str!("../examples/components/resources.walz");
#[test]
fn infers_bound_resource_interfaces_and_native_own_borrow_exports() {
    let built = walz::composition::compile(SOURCE, &Default::default()).unwrap();
    assert!(
        built
            .wit
            .contains("package example:resource-composition@0.1.0")
    );
    assert!(built.wit.contains("borrow<file>"), "{}", built.wit);
    assert!(
        built.wit.contains("import test:resources/host"),
        "{}",
        built.wit
    );
    assert!(
        !built.wit.contains("resource-0"),
        "do not fabricate a second nominal resource: {}",
        built.wit
    );
    assert!(built.wat.contains("canon resource.drop"));
    wit_parser::Resolve::default()
        .push_str("world.wit", &built.wit)
        .expect("generated WIT must parse independently");
    assert!(!built.wat.contains("walz:runtime/"));
    assert_eq!(built.manifest["bindings"][0]["kind"], "instance");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.wit");
    std::fs::write(&path, &built.wit).unwrap();
    built.interface.validate_world(&path, Some("app")).unwrap();
    std::fs::write(&path, built.wit.replace("borrow<file>", "own<file>")).unwrap();
    assert!(built.interface.validate_world(&path, Some("app")).is_err());
}

#[test]
fn resource_binding_inference_keeps_identity_ownership_and_effect_gates() {
    for source in [
        SOURCE.replace("test:resources/host/file", "other/file"),
        SOURCE.replace("(@move $f)", "$f"),
        SOURCE.replace("(@effect host reads)", "(@effect pure)"),
        SOURCE.replace(
            "(param $f $borrow<$File>) (result i32)))",
            "(param $f i32) (result i32)))",
        ),
        SOURCE.replace("[resource-drop]file", "close-file"),
        SOURCE.replace(
            "(@resource.close (@move $f))",
            "(block (@resource.close (@move $f)) (@resource.close (@move $f)))",
        ),
    ] {
        assert!(walz::compile(&source).is_err(), "accepted {source}");
    }
}

#[test]
fn instance_ports_cannot_be_rebound_or_override_fixed_host_contracts() {
    for source in [
        SOURCE.replace(
            "(@expose \"make\"",
            "(@bind \"test:resources/host\" (instance $files)) (@expose \"make\"",
        ),
        SOURCE.replace(
            "(@expose \"make\"",
            "(@bind \"test:resources/host\" \"read\" $read) (@expose \"make\"",
        ),
        SOURCE.replace("test:resources/host", "walz:host/event-loop@0.1.0"),
    ] {
        assert!(walz::compile(&source).is_err());
    }
}

fn projected_source(source: &str) -> String {
    source
        .replace(
            "  (@lib",
            r#"  (instance $selected
      (export "item" (type $files "file"))
      (export "make" (func $files "make"))
      (export "read" (func $files "read")))
      (@lib"#,
        )
        .replace(
            "(import \"test:resources/host\" \"",
            "(import \"selected\" \"",
        )
        .replace("[resource-drop]file", "[resource-drop]item")
        .replace(
            "(@bind \"test:resources/host\" (instance $files))",
            "(@bind \"selected\" (instance $selected))",
        )
}

#[test]
fn projected_resource_aliases_keep_the_defining_type_and_select_only_declared_imports() {
    let source = projected_source(SOURCE);
    let built = walz::composition::compile(&source, &Default::default()).unwrap();
    assert!(built.wit.contains("borrow<file>"), "{}", built.wit);
    assert!(!built.wit.contains("resource-0"));
    assert!(
        walz::compile(&source.replace("\"read\" (func $read", "\"missing\" (func $read")).is_err()
    );
}

#[test]
fn identically_shaped_but_distinct_resource_definitions_do_not_unify() {
    let source = projected_source(SOURCE)
        .replace(
            "  (instance $selected",
            r#"
      (import "other:resources/host" (instance $other
        (export "file" (type $OtherFile (sub resource)))
        (export "make" (func (result (own $OtherFile))))
        (export "read" (func (param "f" (borrow $OtherFile)) (result s32)))))
      (instance $selected"#,
        )
        .replace("(func $files \"make\")", "(func $other \"make\")")
        .replace("(func $files \"read\")", "(func $other \"read\")");
    let error = walz::compile(&source)
        .err()
        .expect("different nominal resources");
    assert!(
        error.to_string().contains("binding signature mismatch"),
        "{error}"
    );
}

#[test]
fn generic_owned_exports_are_monomorphized_against_the_bound_resource() {
    let source = SOURCE
        .replace(
            "  (@lib",
            r#"(@lib
      (func $identity (@type.params $T) (param $f $T) (result $T) (@effect pure) (@move $f))"#,
        )
        .replace(
            "(@expose \"echo\" $echo)",
            "(@expose \"echo\" (@inst $identity (@resource $File)))",
        );
    let built = walz::composition::compile(&source, &Default::default()).unwrap();
    assert!(
        built.wit.contains("echo: func(f: file) -> file"),
        "{}",
        built.wit
    );
}

#[test]
fn private_resource_providers_keep_their_state_and_do_not_expose_their_handles() {
    let source = include_str!("component-fixtures/composition-private-resources.walz");
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    assert!(built.interface.imports.is_empty());
    assert!(built.wit.contains("run: func() -> s32"));
    assert!(!built.wit.contains("resource file"));
}

#[test]
fn native_resource_instance_reexports_do_not_panic_the_wit_decoder() {
    let source = r#"(component
      (import "test:resources/host" (instance $host (export "file" (type (sub resource)))))
      (export "test:resources/host" (instance $host)))"#;
    let built = walz::compile(source).unwrap();
    assert_eq!(built.wasm, wat::parse_str(source).unwrap());
    assert!(
        walz::composition::inspect(&built.wasm).is_err(),
        "an unsupported WIT projection needs a diagnostic"
    );
}

#[test]
fn wit_type_uses_do_not_collide_with_functions_and_local_alias_names_are_not_ports() {
    let source = SOURCE.replace("(@expose \"make\" $make)", "(@expose \"file\" $make)");
    let built = walz::composition::compile(&source, &Default::default()).unwrap();
    assert!(built.wit.contains("file as file-a1"), "{}", built.wit);
    assert!(
        built.wit.contains("export file: func() -> file-a1;"),
        "{}",
        built.wit
    );
    let renamed = built.wit.replace("file-a1", "item");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.wit");
    std::fs::write(&path, renamed).unwrap();
    built.interface.validate_world(&path, Some("app")).unwrap();
}

#[test]
fn wit_scope_projection_keeps_same_named_import_and_export_signatures_separate() {
    let source = SOURCE.replacen(
        "(component",
        r#"(component (import "read" (func (param "v" u32) (result u32)))"#,
        1,
    );
    let built = walz::composition::compile(&source, &Default::default()).unwrap();
    assert!(
        built.wit.contains("import read: func(v: u32) -> u32;"),
        "{}",
        built.wit
    );
    assert!(
        built
            .wit
            .contains("export read: func(f: borrow<file>) -> s32;"),
        "{}",
        built.wit
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.wit");
    std::fs::write(&path, &built.wit).unwrap();
    built.interface.validate_world(&path, Some("app")).unwrap();
}
