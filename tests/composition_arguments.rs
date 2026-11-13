fn compile(source: &str) -> walz::composition::Component {
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&built.wasm)
        .unwrap();
    assert!(
        built.interface.unsupported.is_none(),
        "{:?}",
        built.interface.unsupported
    );
    built
}
const ECHO: &str = r#"(component $Echo (@type.params $T) (@where (shared $T))
    (@lib (func $echo (param $v $T) (result $T) (@effect pure) $v))
    (@expose "echo" $echo))"#;

#[test]
fn source_nominals_are_private_type_definitions_with_stable_argument_identity() {
    let source = format!(
        r#"(component
      (@lib
        (type $A (struct (field $value i32)))
        (type $B (struct (field $value i32)))
        (@abi.type $A (record "value")) (@abi.type $B (record "value"))
        (@type.alias $Alias (ref $A)))
      {ECHO}
      (instance $a (instantiate (@inst $Echo (ref $A))))
      (instance $alias (instantiate $Echo<$Alias>))
      (instance $b (instantiate (@inst $Echo (ref $B))))
      (export "a" (instance $a)) (export "alias" (instance $alias))
      (export "b" (instance $b)))"#
    );
    let built = compile(&source);
    assert_eq!(
        built.wat.matches("(component $Echo.mono").count(),
        2,
        "{}",
        built.wat
    );
    assert!(built.wit.contains("value: s32"), "{}", built.wit);
    assert!(!built.wat.contains("@type.params"));
}

#[test]
fn nominal_closures_keep_abi_annotations_and_reject_resource_authority() {
    let source = format!(
        r#"(component
      (@lib (type $U (@newtype i32)) (@abi.type $U (u32))
        (type $Choice (@variant (none) (value $U)))
        (@abi.type $Choice (variant "none" "value")))
      {ECHO}
      (instance $u (instantiate $Echo<$U>))
      (instance $v (instantiate (@inst $Echo (ref $Choice))))
      (export "u" (instance $u)) (export "v" (instance $v)))"#
    );
    let built = compile(&source);
    assert!(
        built.wit.contains("echo: func(v: u32) -> u32"),
        "{}",
        built.wit
    );
    assert!(built.wit.contains("value(u32)"), "{}", built.wit);
    let resource = format!(
        r#"(component
      (@lib (import "host" "drop" (func $drop (param $v i32)))
        (type $File (@resource "test:files/file" $drop)))
      {ECHO} (instance (instantiate $Echo<(@resource $File)>)))"#
    );
    let error = walz::compile(&resource)
        .err()
        .expect("resource identity requires an instance port");
    assert_eq!(error.diagnostic().code, "E_COMPONENT_GENERIC", "{error}");
}

#[test]
fn constructor_queries_resolve_from_earlier_native_connections_without_a_runtime_binding() {
    let source = format!(
        r#"(component
      (import "test:queries/host" (instance $host
        (type $Def (record (field "value" u32)))
        (export "item" (type $Item (eq $Def)))
        (export "echo" (func (param "v" (option $Item)) (result (option $Item))))))
      (@lib (@type.alias $Input (@component.type "host" (param "echo" 0))))
      {ECHO}
      (instance $a (instantiate $Echo<$Input>))
      (instance $b (instantiate $Echo<(@component.type "host" (result "echo"))>))
      (@bind "host" (instance $host))
      (export "a" (instance $a)) (export "b" (instance $b)))"#
    );
    let built = compile(&source);
    assert!(built.wit.contains("value: u32"), "{}", built.wit);
    assert!(built.wit.contains("option<"), "{}", built.wit);
    assert_eq!(
        built.wat.matches("(component $Echo.mono").count(),
        1,
        "{}",
        built.wat
    );
    assert!(!built.wat.contains("@component.type"));
}

#[test]
fn queries_can_depend_on_preceding_specializations_but_cannot_run_initializers() {
    let source = format!(
        r#"(component
      {ECHO}
      (instance $first (instantiate $Echo<i64>))
      (alias export $first "echo" (func $first-echo))
      (@bind "first" "call" $first-echo)
      (instance $second (instantiate $Echo<(@component.type "first" (result "call"))>))
      (export "second" (instance $second)))"#
    );
    let built = compile(&source);
    assert!(
        built.wit.contains("echo: func(v: s64) -> s64"),
        "{}",
        built.wit
    );
    assert_eq!(built.wat.matches("(component $Echo.mono").count(), 1);
    // No runtime code is executed while reading a producer's validated types.
    let trapping = source.replace(
        ECHO,
        &format!(
            r#"(core module $trap
      (func $start unreachable) (start $start))
      (core instance $trap-instance (instantiate $trap)) {ECHO}"#
        ),
    );
    compile(&trapping);
    let forward = source
        .replace("(instance $first (instantiate $Echo<i64>))", "")
        .replace(
            "(export \"second\"",
            "(instance $first (instantiate $Echo<i64>)) (export \"second\"",
        );
    let error = walz::compile(&forward).err().unwrap();
    assert_eq!(error.diagnostic().code, "E_COMPONENT_TYPE_PHASE", "{error}");
    assert!(error.diagnostic().primary.is_some());
    assert!(!error.to_string().contains("walz-deferred"));
}

#[test]
fn nominal_storage_keeps_mutability_recursion_and_heap_operands_inside_each_component() {
    let source = r#"(component
      (@lib (type $Cell (struct (field $n (mut i32))))
        (type $Node (struct (field $next (ref null $Node)))))
      (component $CellCounter (@type.params $T)
        (@lib (func $count (result i32) (@effect reads writes allocates)
          (@let $cell (struct.new $T 41)
            (block (result i32) (struct.set $T $n $cell 42) (struct.get $T $n $cell)))))
        (@expose "count" $count))
      (component $Private (@type.params $T)
        (@lib (func $unused (param $v $T) (nop))
          (func $answer (result i32) (@effect pure) 42)) (@expose "answer" $answer))
      (instance $c (instantiate (@inst $CellCounter (ref $Cell))))
      (instance $n (instantiate (@inst $Private (ref null $Node))))
      (export "counter" (instance $c)) (export "node" (instance $n)))"#;
    compile(source);
    let error =
        walz::compile(&source.replace("(@effect reads writes allocates)", "(@effect pure)"))
            .err()
            .unwrap();
    assert_eq!(error.diagnostic().code, "E_EFFECT_CONTRACT", "{error}");
}

#[test]
fn opaque_module_representations_cannot_be_copied_into_a_constructor_scope() {
    let dir = tempfile::tempdir().unwrap();
    for visibility in ["type", "opaque"] {
        std::fs::write(
            dir.path().join("types.walz"),
            format!(
                r#"(@lib
          (type $Secret (struct (field $n i32))) (@export ({visibility} $Secret)))"#
            ),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("main.walz"),
            r#"(component
          (@use "./types.walz" (type $Secret))
          (component $Read (@type.params $T)
            (@lib (func $read (param $v $T) (result i32) (@effect pure) (struct.get $T $n $v)))
            (@expose "read" $read))
          (instance (instantiate (@inst $Read (ref $Secret)))))"#,
        )
        .unwrap();
        let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
        let error = walz::compile(loaded.text()).err().unwrap();
        if visibility == "opaque" {
            assert!(error.to_string().contains("opaque"), "{error}");
            assert!(error.diagnostic().primary.is_some());
        } else {
            assert_eq!(error.diagnostic().code, "E_ABI_TYPE", "{error}");
        }
    }
}

#[test]
fn constructor_queries_recheck_actual_bounds_and_allow_unrelated_later_aliases() {
    let source = format!(
        r#"(component
      (import "test:queries/host" (instance $host
        (export "number" (func (result u32)))))
      (@lib (@type.alias $Number (@component.type "host" (result "number")))
        (@type.alias $Later (@component.type "later" (result "echo"))))
      {ECHO}
      (instance $a (instantiate $Echo<$Number>))
      (alias export $a "echo" (func $later))
      (@bind "later" "echo" $later) (@bind "host" (instance $host))
      (export "a" (instance $a)))"#
    );
    compile(&source);
    let source = source.replace("(@where (shared $T))", "(@where (Add $T))");
    let error = walz::compile(&source).err().unwrap();
    assert_eq!(error.diagnostic().code, "E_GENERIC_BOUND", "{error}");
}

#[test]
fn staged_nominal_sample_has_a_roundtrippable_concrete_wit_contract() {
    let source = include_str!("../examples/components/typed-constructors.walz");
    let built = compile(source);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.wit");
    std::fs::write(&path, &built.wit).unwrap();
    built.interface.validate_world(&path, Some("app")).unwrap();
    assert!(built.wit.contains("value: u32"), "{}", built.wit);
    assert!(built.wit.contains("future<"));
    assert!(built.wit.contains("stream<"));
    let error = walz::compile(&source.replace("(@move $value)", "$value"))
        .err()
        .unwrap();
    assert!(error.to_string().contains("explicit move"), "{error}");
}

#[test]
fn private_unit_arguments_and_constructor_diagnostics_use_public_names() {
    let source = r#"(component (component $Private (@type.params $T)
      (@lib (func $answer (result i32) (@effect pure) 42)) (@expose "answer" $answer))
      (instance $v (instantiate $Private<@unit>)) (export "v" (instance $v)))"#;
    compile(source);
    let source = r#"(component (@lib (type $Left (@newtype i32)) (type $Right (@newtype i32)))
      (component $Bad (@type.params $T $U)
        (@lib (func $wrong (param $v $U) (result $T) (@effect pure) $v)) (@expose "wrong" $wrong))
      (instance (instantiate $Bad<$Left,$Right>)))"#;
    let error = walz::compile(source).err().unwrap();
    assert_eq!(error.diagnostic().code, "E_TYPE_MISMATCH", "{error}");
    assert!(
        !error.to_string().contains("walz-constructor-type"),
        "{error}"
    );
    assert!(error.to_string().contains("$Left"), "{error}");
}

#[test]
fn value_ascriptions_preserve_external_resource_identity_and_scoped_destruction() {
    let built = compile(include_str!(
        "component-fixtures/composition-nominal-resources.walz"
    ));
    assert!(built.wit.contains("borrow<"), "{}", built.wit);
    assert!(built.wat.contains("canon resource.drop"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.wit");
    std::fs::write(&path, &built.wit).unwrap();
    built.interface.validate_world(&path, Some("app")).unwrap();
}

#[test]
fn phantom_generic_nominals_keep_argument_identity_across_separate_probes() {
    let source = r#"(component
      (@lib (type $Phantom (@type.params $T) (struct (field $value i32))))
      (component $Private (@type.params $T)
        (@lib (func $private (param $v $T) (nop))
          (func $answer (result i32) (@effect pure) 42)) (@expose "answer" $answer))
      (instance $a (instantiate $Private<(ref $Phantom<i32>)>))
      (instance $b (instantiate $Private<(ref $Phantom<i64>)>))
      (export "a" (instance $a)) (export "b" (instance $b)))"#;
    let built = compile(source);
    assert_eq!(
        built.wat.matches("(component $Private.mono").count(),
        2,
        "{}",
        built.wat
    );
}
