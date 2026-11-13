fn valid(bytes: &[u8]) {
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .unwrap();
}
const SOURCE: &str = r#"(component
  (import "test:queries/host" (instance $host
    (type $ItemDef (record (field "value" s32) (field "label" string)))
    (export "item" (type $Item (eq $ItemDef)))
    (export "echo" (func (param "value" (option (list s32))) (result (option (list s32)))))
    (export "number" (func (param "value" u32) (result u32)))
    (export "item-value" (func (param "value" $Item) (result s32)))))
  (@lib
    (@type.alias $Input (@component.type "host" (param "echo" "value")))
    (@type.alias $Output (@component.type "host" (result "echo")))
    (@type.alias $Number (@component.type "host" (param "number" 0)))
    (@type.alias $Item (@component.type "host" (type "item")))
    (import "host" "echo" (func $host-echo (param $v $Input) (result $Output)))
    (import "host" "number" (func $host-number (param $v $Number) (result $Number)))
    (import "host" "item-value" (func $host-item-value (param $v $Item) (result i32)))
    (func $echo (param $v $Input) (result $Output) (@effect host) (call $host-echo $v))
    (func $number (param $v $Number) (result $Number) (@effect host) (call $host-number $v))
    (func $read (param $v $Item) (result i32) (@effect host) (call $host-item-value $v))
    (func $element (param $v (@component.type "host" (result "echo") option.element list.element))
      (result i32) (@effect pure) $v)
    (func $field (param $v (@component.type "host" (type "item") (field "value")))
      (result i32) (@effect pure) $v))
  (@bind "host" (instance $host))
  (@expose "echo" $echo) (@expose "number" $number) (@expose "read" $read)
  (@expose "element" $element) (@expose "field" $field))"#;

#[test]
fn bound_type_queries_preserve_value_shapes_unsigned_abi_and_named_records() {
    let built = walz::composition::compile(SOURCE, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(
        built.wit.contains("number: func(v: u32) -> u32"),
        "{}",
        built.wit
    );
    assert!(built.wit.contains("option<list<s32>>"), "{}", built.wit);
    assert!(
        built.wit.contains("element: func(v: s32) -> s32"),
        "{}",
        built.wit
    );
    assert!(built.wit.contains("label: string"), "{}", built.wit);
    assert!(!built.wat.contains("@component.type"));
}

#[test]
fn query_errors_are_local_and_do_not_add_ports_or_change_effects() {
    for (before, after) in [
        (r#""host" (result "echo")"#, r#""missing" (result "echo")"#),
        (r#"(param "echo" "value")"#, r#"(param "echo" "missing")"#),
        (r#"(param "number" 0)"#, r#"(param "number" 1)"#),
        (
            r#"(type "item") (field "value")"#,
            r#"(type "item") (field "absent")"#,
        ),
        ("option.element list.element", "option.element result.ok"),
    ] {
        let source = SOURCE.replace(before, after);
        let error = walz::compile(&source)
            .err()
            .expect("must reject invalid query");
        assert_eq!(error.diagnostic().code, "E_COMPONENT_TYPE", "{error}");
        assert!(error.diagnostic().primary.is_some(), "{error}");
    }
    assert!(walz::compile(&SOURCE.replace("(@effect host)", "(@effect pure)")).is_err());
    assert!(
        walz::compile(&SOURCE.replace(
            "(call $host-number $v)",
            "(call $host-number (i32.const -1))"
        ))
        .is_err()
    );
    let unrelated = SOURCE.replace("(@bind \"host\" (instance $host))", "");
    assert!(
        walz::compile(&unrelated).is_err(),
        "queries need explicit connections"
    );
}

#[test]
fn single_function_bindings_can_project_signatures_without_granting_an_interface() {
    let source = r#"(component
      (import "echo" (func $echo (param "value" string) (result string)))
      (@lib
        (import "echo" "call" (func $echo
          (param $value (@component.type "echo" (param "call" 0)))
          (result (@component.type "echo" (result "call")))))
        (func $run (param $value (@component.type "echo" (param "call" "value")))
          (result (@component.type "echo" (result "call"))) (@effect host) (call $echo $value)))
      (@bind "echo" "call" $echo) (@expose "run" $run))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    assert!(
        built.wit.contains("run: func(value: string) -> string"),
        "{}",
        built.wit
    );
}

#[test]
fn query_annotations_are_reserved_in_core_sources_but_opaque_metadata_is_not_read() {
    let error = walz::compile(r#"(module (@type.alias $T (@component.type "x" (result "f"))))"#)
        .err()
        .unwrap();
    assert_eq!(error.diagnostic().code, "E_COMPONENT_TYPE_CONTEXT");
    let raw = r#"(module (func (export "f") (result i32) i32.const 42) (@component.type "x" (result "f")))"#;
    assert!(
        walz::compile(raw).is_err(),
        "reserved queries must not disappear on a native/opaque path"
    );
    let opaque = r#"(component (@note (@component.type "x" (result "f"))))"#;
    assert_eq!(
        walz::compile(opaque).unwrap().wasm,
        wat::parse_str(opaque).unwrap()
    );
    let source = r#"(component (@custom "opaque" "(@component.type bogus)"))"#;
    assert_eq!(
        walz::compile(source).unwrap().wasm,
        wat::parse_str(source).unwrap()
    );
}

#[test]
fn resource_queries_use_explicit_owners_and_borrows_without_fabricating_destructors() {
    let source = include_str!("../examples/components/resources.walz")
        .replace(
            "(result (@resource $File))",
            r#"(result (@component.type "test:resources/host" (result "make")))"#,
        )
        .replace(
            "(param $f $borrow<$File>)",
            r#"(param $f (@component.type "test:resources/host" (param "read" 0)))"#,
        );
    let built = walz::composition::compile(&source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(built.wit.contains("borrow<file>"));
    assert!(built.wat.contains("canon resource.drop"));
    assert!(walz::compile(&source.replace("(@move $f)", "$f")).is_err());
    let missing = source.replace(
        r#"(type $File (@resource "test:resources/host/file" $drop))"#,
        "",
    );
    let error = walz::compile(&missing).err().unwrap();
    assert_eq!(error.diagnostic().code, "E_COMPONENT_TYPE");
    assert!(error.to_string().contains("explicit"), "{error}");
}

#[test]
fn named_types_without_live_host_calls_retain_only_the_required_type_port() {
    let source = r#"(component
      (import "test:queries/host" (instance $host
        (type $Def (record (field "value" s32)))
        (export "item" (type $Item (eq $Def)))))
      (@lib
        (func $echo (param $v (@component.type "host" (type "item")))
          (result (@component.type "host" (type "item"))) (@effect pure) $v))
      (@bind "host" (instance $host)) (@expose "echo" $echo))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(built.wit.contains("record item"), "{}", built.wit);
    assert!(!built.wit.contains("resource-"));
}

#[test]
fn query_aliases_compose_with_function_generics_but_do_not_cycle_constructor_arguments() {
    let source = SOURCE
        .replace(
            "(@lib",
            r#"(@lib (func $id (@type.params $T) (param $v $T) (result $T) (@effect pure) $v)"#,
        )
        .replace(
            "(@expose \"echo\" $echo)",
            r#"(@expose "echo" $id<$Input>)"#,
        )
        .replacen(
            "(component",
            r#"(component
          (component $Unused (@type.params $T)) (instance (instantiate $Unused<i32>))"#,
            1,
        );
    let built = walz::compile(&source).unwrap();
    valid(&built.wasm);
    let cyclic = source.replace("$Unused<i32>", "$Unused<$Input>");
    let error = walz::compile(&cyclic).err().unwrap();
    assert_eq!(error.diagnostic().code, "E_COMPONENT_TYPE_PHASE", "{error}");
    assert!(!error.to_string().contains("walz-deferred"), "{error}");
    let invalid_unused = source.replace(r#"(param "echo" "value")"#, r#"(param "echo" "missing")"#);
    assert_eq!(
        walz::compile(&invalid_unused)
            .err()
            .unwrap()
            .diagnostic()
            .code,
        "E_COMPONENT_TYPE"
    );
}

#[test]
fn static_library_queries_keep_their_type_helpers_private_and_local() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("impl.walz"),
        r#"(@lib
      (@type.alias $Value (@component.type "host" (param "echo" 0)))
      (import "host" "echo" (func $host (param $v $Value) (result $Value)))
      (func $run (param $v $Value) (result $Value) (@effect host) (call $host $v))
      (@export (func $run)))"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("main.walz"),
        r#"(component
      (import "echo" (func $echo (param "value" string) (result string)))
      (@use "./impl.walz" (func $run)) (@bind "host" "echo" $echo) (@expose "run" $run))"#,
    )
    .unwrap();
    let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
    valid(&walz::compile(loaded.text()).unwrap().wasm);
}

#[test]
fn nested_result_variant_tuple_and_future_projections_keep_unit_and_unsigned_types() {
    let source = r#"(component
      (import "test:projections/host" (instance $host
        (type $TripleDef (tuple s32 string u32))
        (export "triple" (type $Triple (eq $TripleDef)))
        (type $Ready (future string))
        (type $ChoiceDef (variant (case "ready" $Ready) (case "empty")))
        (export "choice" (type $Choice (eq $ChoiceDef)))
        (export "lookup" (func (result (result $Triple (error $Choice)))))))
      (@lib
        (@use "walz:core" (type $result))
        (func $index (param $v (@component.type "host" (result "lookup") result.ok (item 2)))
          (result (@component.type "host" (type "triple") (item 2))) (@effect pure) $v)
        (func $text (param $v (@component.type "host" (result "lookup") result.err (case "ready") future.element))
          (result (@component.type "host" (type "triple") (item 1))) (@effect pure) $v)
        (func $unit (result $result<i32, (@component.type "host" (type "choice") (case "empty"))>)
          (@effect allocates) (@result.ok 42)))
      (@bind "host" (instance $host)) (@expose "index" $index) (@expose "text" $text) (@expose "unit" $unit))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(
        built.wit.contains("index: func(v: u32) -> u32"),
        "{}",
        built.wit
    );
    assert!(
        built.wit.contains("unit: func() -> result<s32>;"),
        "{}",
        built.wit
    );
}

#[test]
fn type_only_ports_do_not_shift_unrelated_bound_function_groups() {
    let source = r#"(component
      (import "test:query-types/host" (instance $types
        (type $Def (record (field "value" s32))) (export "item" (type $Item (eq $Def)))))
      (import "plus" (func $plus (param "v" s32) (result s32)))
      (@lib
        (import "zzz" "plus" (func $plus (param $v i32) (result i32)))
        (func $run (param $v i32) (result i32) (@effect host) (call $plus $v))
        (func $echo (param $v (@component.type "aaa" (type "item")))
          (result (@component.type "aaa" (type "item"))) (@effect pure) $v))
      (@bind "aaa" (instance $types)) (@bind "zzz" "plus" $plus)
      (@expose "run" $run) (@expose "echo" $echo))"#;
    valid(&walz::compile(source).unwrap().wasm);
}

#[test]
fn queried_flags_and_enums_need_no_live_host_functions() {
    let source = r#"(component
      (import "test:query-values/host" (instance $host
        (type $FlagsDef (flags "read" "write")) (export "flags" (type $Flags (eq $FlagsDef)))
        (type $EnumDef (enum "first" "second")) (export "choice" (type $Enum (eq $EnumDef)))))
      (@lib
        (func $flags (param $v (@component.type "host" (type "flags")))
          (result (@component.type "host" (type "flags"))) (@effect pure) $v)
        (func $choice (param $v (@component.type "host" (type "choice")))
          (result (@component.type "host" (type "choice"))) (@effect pure) $v))
      (@bind "host" (instance $host)) (@expose "flags" $flags) (@expose "choice" $choice))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(built.wit.contains("flags %flags"), "{}", built.wit);
    assert!(built.wit.contains("enum choice"), "{}", built.wit);
}
