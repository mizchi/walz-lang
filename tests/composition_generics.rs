fn valid(bytes: &[u8]) {
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .unwrap();
}

const SOURCE: &str = r#"(component
  (component $Echo (@type.params $T)
    (@lib (func $echo (param $v $T) (result $T) (@effect pure) $v))
    (@expose "echo" $echo))
  (instance $a (instantiate $Echo<i32>))
  (instance $b (instantiate (@inst $Echo i32)))
  (instance $c (instantiate $Echo<i64>))
  (export "a" (instance $a)) (export "b" (instance $b)) (export "c" (instance $c)))"#;

#[test]
fn explicit_and_angle_constructor_arguments_specialize_without_instantiating_implicitly() {
    let built = walz::composition::compile(SOURCE, &Default::default()).unwrap();
    valid(&built.wasm);
    assert_eq!(
        built.wat.matches("(component $Echo.mono").count(),
        2,
        "{}",
        built.wat
    );
    assert!(
        built.wit.contains("echo: func(v: s32) -> s32"),
        "{}",
        built.wit
    );
    assert!(
        built.wit.contains("echo: func(v: s64) -> s64"),
        "{}",
        built.wit
    );
    assert!(!built.wat.contains("@type.params"));
    assert!(!built.wat.contains("@inst"));
}

#[test]
fn constructor_constraints_and_type_effect_ownership_checks_still_apply() {
    let bounded = SOURCE.replace("(@type.params $T)", "(@type.params $T) (@where (Add $T))");
    valid(&walz::compile(&bounded).unwrap().wasm);
    for (source, code) in [
        (SOURCE.replace("$Echo<i64>", "$Echo"), "E_COMPONENT_GENERIC"),
        (
            SOURCE.replace("$Echo<i64>", "$Echo<i32, i64>"),
            "E_COMPONENT_GENERIC",
        ),
        (
            SOURCE.replace("(@type.params $T)", "(@type.params $T $T)"),
            "E_COMPONENT_GENERIC",
        ),
        (
            SOURCE.replace("$Echo<i64>", "$Echo<$Missing>"),
            "E_COMPONENT_GENERIC",
        ),
        (
            bounded.replace("$Echo<i64>", "$Echo<@string>"),
            "E_GENERIC_BOUND",
        ),
        (
            SOURCE.replace("(@effect pure) $v", "(@effect pure) (i64.const 0)"),
            "E_TYPE_MISMATCH",
        ),
        (
            SOURCE.replace("(@lib", "(@lib (memory $heap 1)")
                .replace("(@effect pure) $v", "(@effect pure) (block (result $T) (i32.store $heap (i32.const 0) (i32.const 1)) $v)"),
            "E_EFFECT_CONTRACT",
        ),
    ] {
        let error = walz::compile(&source)
            .err()
            .unwrap_or_else(|| panic!("must reject invalid specialization ({code}): {source}"));
        assert_eq!(error.diagnostic().code, code, "{error}");
        assert!(
            error.diagnostic().primary.is_some(),
            "a constructor error needs a source location: {error}"
        );
    }
}

#[test]
fn literal_native_component_names_keep_precedence_over_angle_sugar() {
    let source = r#"(component (component $C<i32>) (instance (instantiate $C<i32>)))"#;
    assert_eq!(
        walz::compile(source).unwrap().wasm,
        wat::parse_str(source).unwrap()
    );
}

#[test]
fn source_constructor_imports_keep_generic_parameters_until_specialization() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("echo.walz"),
        r#"(component (@type.params $T)
      (@lib (func $echo (param $v $T) (result $T) (@effect pure) $v))
      (@expose "echo" $echo))"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("main.walz"),
        r#"(component
      (@use "./echo.walz" (component $Echo))
      (instance $a (instantiate $Echo<f64>)) (export "api" (instance $a)))"#,
    )
    .unwrap();
    let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
    let built = walz::composition::compile(loaded.text(), &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(
        built.wit.contains("echo: func(v: f64) -> f64"),
        "{}",
        built.wit
    );
}

#[test]
fn closed_value_arguments_resolve_in_the_callers_explicit_type_scope() {
    let source = SOURCE
        .replacen(
            "(component",
            r#"(component
          (@lib (@use "walz:core" (type $option $maybe) (type $string))
            (@type.alias $Input $maybe<$string>))"#,
            1,
        )
        .replace("$Echo<i32>", "$Echo<$Input>")
        .replace("(@inst $Echo i32)", "(@inst $Echo (@inst $maybe $string))");
    let built = walz::composition::compile(&source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert_eq!(
        built.wat.matches("(component $Echo.mono").count(),
        2,
        "{}",
        built.wat
    );
    assert!(
        built
            .wit
            .contains("echo: func(v: option<string>) -> option<string>"),
        "{}",
        built.wit
    );
    assert!(
        walz::compile(&source.replace("(type $string)", "")).is_err(),
        "an undeclared name is not an implicit core import"
    );
}

#[test]
fn nested_binders_and_term_names_do_not_capture_constructor_parameters() {
    let source = r#"(component
      (component $Outer (@type.params $T)
        (component $Inner (@type.params $U)
          (@lib
            (func $echo (param $T $T) (result $T) (@effect pure) $T)
            (func $id (@type.params $T) (param $v $T) (result $T) (@effect pure) $v))
          (@expose "outer" $echo) (@expose "inner" $id<$U>))
        (instance $inner (instantiate $Inner<i64>))
        (alias export $inner "outer" (func $outer))
        (alias export $inner "inner" (func $inner-f))
        (export "outer" (func $outer)) (export "inner" (func $inner-f)))
      (instance $outer (instantiate $Outer<i32>)) (export "outer" (instance $outer)))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(
        built.wit.contains("outer: func(p0: s32) -> s32"),
        "{}",
        built.wit
    );
    assert!(
        built.wit.contains("inner: func(v: s64) -> s64"),
        "{}",
        built.wit
    );
}

#[test]
fn owned_endpoints_still_require_moves_and_shared_bounds_reject_them() {
    let source = r#"(component
      (@lib (@use "walz:core" (type $future)))
      (component $Transfer (@type.params $T)
        (@lib (func $transfer (param $v $T) (result $T) (@effect pure) (@move $v)))
        (@expose "transfer" $transfer))
      (instance $transfer (instantiate $Transfer<$future<i32>>))
      (export "api" (instance $transfer)))"#;
    valid(&walz::compile(source).unwrap().wasm);
    let missing_move = walz::compile(&source.replace("(@move $v)", "$v"))
        .err()
        .unwrap();
    assert!(
        missing_move
            .diagnostic()
            .message
            .contains("requires explicit move"),
        "{missing_move}"
    );
    let shared = source.replace(
        "(@type.params $T)",
        "(@type.params $T) (@where (shared $T))",
    );
    assert_eq!(
        walz::compile(&shared).err().unwrap().diagnostic().code,
        "E_GENERIC_BOUND"
    );
}

#[test]
fn unannotated_gc_arguments_still_need_a_public_value_adapter() {
    for ty in ["(ref $S)", "(ref null $S)"] {
        let source = SOURCE
            .replacen(
                "(component",
                "(component (@lib (type $S (struct)) (type $New (@newtype i32)))",
                1,
            )
            .replace("$Echo<i64>", &format!("(@inst $Echo {ty})"));
        let error = walz::compile(&source).err().unwrap();
        assert_eq!(error.diagnostic().code, "E_ABI_TYPE", "{error}");
    }
}

#[test]
fn private_closed_types_do_not_need_a_public_wit_representation() {
    let source = r#"(component
      (@lib (@use "walz:core" (type $box) (type $task)))
      (component $Private (@type.params $T)
        (@lib
          (func $private (result $T) (unreachable))
          (func $answer (result i32) (@effect pure) 42))
        (@expose "answer" $answer))
      (instance $simd (instantiate $Private<v128>))
      (instance $owned (instantiate $Private<$box<i32>>))
      (instance $task (instantiate $Private<$task<i32>>))
      (export "simd" (instance $simd)) (export "owned" (instance $owned)) (export "task" (instance $task))
      (component $VectorStore (@type.params $T)
        (@lib (type $Store (array $T))
          (func $length (result i32) (@effect allocates)
            (array.len (array.new $Store (v128.const i32x4 1 2 3 4) (i32.const 4)))))
        (@expose "length" $length))
      (instance $vectors (instantiate $VectorStore<v128>)) (export "vectors" (instance $vectors)))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(built.wit.contains("answer: func() -> s32"), "{}", built.wit);
    assert_eq!(
        walz::compile(&SOURCE.replace("$Echo<i64>", "$Echo<v128>"))
            .err()
            .unwrap()
            .diagnostic()
            .code,
        "E_ABI_TYPE"
    );
}

#[test]
fn same_specialized_definition_still_generates_distinct_resource_identities() {
    let source = SOURCE.replace(
        "(@lib (func $echo",
        r#"(type $Handle (resource (rep i32)))
      (instance $api (export "handle" (type $Handle)))
      (export "api" (instance $api)) (@lib (func $echo"#,
    );
    let built = walz::compile(&source).unwrap();
    let types = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&built.wasm)
        .unwrap();
    use wasmparser::component_types::{ComponentAnyTypeId, ComponentEntityType};
    let resources = ["a", "b"].map(|name| {
        let ComponentEntityType::Instance(id) = types.component_item_for_export(name).unwrap().ty
        else {
            panic!("instance")
        };
        let ComponentEntityType::Instance(id) = types[id].exports["api"].ty else {
            panic!("api instance")
        };
        let ComponentEntityType::Type {
            referenced: ComponentAnyTypeId::Resource(r),
            ..
        } = types[id].exports["handle"].ty
        else {
            panic!("resource")
        };
        r.resource()
    });
    assert_ne!(
        resources[0], resources[1],
        "instantiation is generative even when the definition is shared"
    );
}

#[test]
fn multiple_parameters_and_literal_inner_names_preserve_their_scopes() {
    let source = r#"(component
      (component $Pair (@type.params $T $U) (@where (shared $T $U))
        (component $Literal<i32>) (instance (instantiate $Literal<i32>))
        (@lib (@use "walz:core" (type $tuple))
          (func $pair (param $a $T) (param $b $U) (result $tuple<$T, $U>)
            (@effect allocates) (@tuple.new $T $U $a $b)))
        (@expose "pair" $pair))
      (instance $pair (instantiate $Pair<i32, i64>)) (export "api" (instance $pair)))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    assert!(
        built
            .wit
            .contains("pair: func(a: s32, b: s64) -> tuple<s32, s64>"),
        "{}",
        built.wit
    );
}

#[test]
fn static_file_type_aliases_are_resolved_without_copying_nominal_definitions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("types.walz"),
        r#"(@lib
      (@use "walz:core" (type $option) (type $string))
      (@type.alias $Input $option<$string>) (@export (type $Input)))"#,
    )
    .unwrap();
    let source = SOURCE
        .replacen(
            "(component",
            r#"(component (@use "./types.walz" (type $Input))"#,
            1,
        )
        .replace("$Echo<i64>", "$Echo<$Input>");
    std::fs::write(dir.path().join("main.walz"), source).unwrap();
    let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
    let built = walz::composition::compile(loaded.text(), &Default::default()).unwrap();
    assert!(
        built
            .wit
            .contains("echo: func(v: option<string>) -> option<string>"),
        "{}",
        built.wit
    );
}

#[test]
fn generic_constructor_exports_do_not_instantiate_and_binaries_are_not_templates() {
    let source = r#"(component (component $Echo (@type.params $T)
      (@lib (func $echo (param $v $T) (result $T) (@effect pure) $v)) (@expose "echo" $echo))
      (export "constructor" (component $Echo<i32>)))"#;
    let built = walz::compile(source).unwrap();
    valid(&built.wasm);
    // The specialized implementation may contain an internal instance, but
    // the root exports a definition with no root-level instantiate operation.
    let normalized = wasmprinter::print_bytes(&built.wasm).unwrap();
    let mut depth = 0;
    let mut roots = 0;
    for p in wasmparser::Parser::new(0).parse_all(&built.wasm) {
        match p.unwrap() {
            wasmparser::Payload::Version { .. } => depth += 1,
            wasmparser::Payload::End(_) => depth -= 1,
            wasmparser::Payload::ComponentInstanceSection(_) if depth == 1 => roots += 1,
            _ => {}
        }
    }
    assert_eq!(roots, 0, "{normalized}");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("unit.wasm"),
        wat::parse_str("(component)").unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("main.walz"),
        r#"(component
      (@use "./unit.wasm" (component $Binary)) (instance (instantiate $Binary<i32>)))"#,
    )
    .unwrap();
    let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
    assert_eq!(
        walz::compile(loaded.text())
            .err()
            .unwrap()
            .diagnostic()
            .code,
        "E_COMPONENT_GENERIC"
    );
}

#[test]
fn native_type_definitions_and_annotations_do_not_become_walz_type_arguments() {
    let source = SOURCE.replacen(
        "(component",
        r#"(component
      (type $Signature (func (param "n" u32)))
      (@custom "opaque" "(@inst $Echo v128)")"#,
        1,
    );
    valid(&walz::compile(&source).unwrap().wasm);
}

#[test]
fn numeric_component_references_cannot_silently_select_a_generated_definition() {
    let source = r#"(component
      (component $Generic (@type.params $T))
      (instance $first (instantiate $Generic<i32>))
      (component $Native) (instance (instantiate 0)))"#;
    let error = walz::compile(source)
        .err()
        .expect("numeric indices would change meaning");
    assert_eq!(error.diagnostic().code, "E_COMPONENT_GENERIC");
    assert!(error.to_string().contains("named Component"), "{error}");
    let with_outer = source.replace(
        "(instance (instantiate 0))",
        "(component $Outer (alias outer 1 0 (component $Alias)))",
    );
    assert!(walz::compile(&with_outer).is_err());
    valid(
        &walz::compile("(component (component) (instance (instantiate 0)))")
            .unwrap()
            .wasm,
    );
}

#[test]
fn specialization_diagnostics_point_to_both_source_files_without_internal_witness_names() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("echo.walz"),
        r#"(component (@type.params $T)
      (@lib (func $echo (param $v $T) (result $T) (@effect pure) (i32.const 0)))
      (@expose "echo" $echo))"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("main.walz"),
        r#"(component
      (@use "./echo.walz" (component $Echo)) (instance (instantiate $Echo<f64>)))"#,
    )
    .unwrap();
    let loaded = walz::source::load(dir.path().join("main.walz")).unwrap();
    let error = loaded.diagnostic(walz::compile(loaded.text()).err().unwrap());
    assert_eq!(error.diagnostic().code, "E_TYPE_MISMATCH");
    assert_eq!(
        error
            .diagnostic()
            .primary
            .as_ref()
            .unwrap()
            .location
            .path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap(),
        "echo.walz"
    );
    assert!(
        error.diagnostic().related.iter().any(|l| l
            .location
            .path
            .as_ref()
            .is_some_and(|p| p.ends_with("main.walz"))),
        "{error}"
    );
    let bounded = SOURCE
        .replace("(@type.params $T)", "(@type.params $T) (@where (Add $T))")
        .replace("$Echo<i64>", "$Echo<@string>");
    let error = walz::compile(&bounded).err().unwrap();
    assert!(error.to_string().contains("$Echo<"), "{error}");
    assert!(
        !error.to_string().contains("walz-constructor-argument"),
        "{error}"
    );
    for declaration in [
        "(type $T (struct))",
        r#"(@use "walz:core" (type $string $T))"#,
    ] {
        let source = SOURCE.replace("(@lib", &format!("(@lib {declaration}"));
        assert_eq!(
            walz::compile(&source).err().unwrap().diagnostic().code,
            "E_COMPONENT_GENERIC"
        );
    }
}

#[test]
fn constructor_parameters_work_in_typed_lets_and_function_applications() {
    let source = r#"(component
      (component $Add (@type.params $T) (@where (Add $T))
        (@lib
          (func $identity (@type.params $U) (param $v $U) (result $U) (@effect pure) $v)
          (func $add (param $a $T) (param $b $T) (result $T) (@effect pure)
            (@let $sum (type $T) (+ $a $b) (call $identity<$T> $sum))))
        (@expose "add" $add) (@expose "identity" $identity<$T>))
      (instance $add (instantiate $Add<i64>)) (export "api" (instance $add)))"#;
    let built = walz::composition::compile(source, &Default::default()).unwrap();
    valid(&built.wasm);
    assert!(
        built.wit.contains("add: func(a: s64, b: s64) -> s64"),
        "{}",
        built.wit
    );
    assert!(
        built.wit.contains("identity: func(v: s64) -> s64"),
        "{}",
        built.wit
    );
}

#[test]
fn native_core_literal_angle_names_survive_constructor_expansion() {
    let source = SOURCE.replacen(
        "(component",
        r#"(component
      (core module $Native
        (func $literal<i32> (result i32) (i32.const 42))
        (func (export "run") (result i32) (call $literal<i32>)))
      (core instance $native (instantiate $Native))
      (func (export "run") (result s32) (canon lift (core func $native "run")))"#,
        1,
    );
    valid(&walz::compile(&source).unwrap().wasm);
}
