use std::process::Command;

#[test]
fn projections_do_not_depend_on_normalized_type_spellings() {
    let source = r#"(module
      (@use "walz:core" (type $option) (type $result) (type $string))
      (@macro $unwrap
        (rule (?value:expr)
          (infer (?T (option.element ?value)))
          (@match (@inst $option ?T) (result ?T) ?value
            (@case none () (unreachable)) (@case some ($value) $value))))
      (@macro $first
        (rule (?value:expr)
          (infer (?T (array.element ?value)) (?A (ref.heap ?value)))
          (array.get ?A ?value 0)))
      (type $Values (array i64))
      (func $main (export "main") (result i64) (@effect reads allocates)
        (+ (@$unwrap (@option.some i64 20))
           (@$first (array.new_fixed $Values 1 22)))))"#;
    let report = walz::macros::report(source, &Default::default()).unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["type_schema"], "walz-type-info-v1");
    assert_eq!(json["expansions"][0]["types"]["?T"]["kind"], "scalar");
    assert_eq!(json["expansions"][0]["types"]["?T"]["name"], "i64");
    let compiled = walz::compile(source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.wasm");
    std::fs::write(&file, compiled.wasm).unwrap();
    let out = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y",
            "--invoke",
            "main",
        ])
        .arg(file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "42");
}

#[test]
fn structured_type_information_uses_public_constructors() {
    let source = r#"(module
      (@use "walz:core" (type $option) (type $result) (type $string))
      (@macro $identity (rule (?x:expr) (infer (?T ?x)) ?x))
      (func $f (result (@inst $option (@inst $result i32 (@tuple i64 $string))))
        (@$identity (@option.none (@inst $result i32 (@tuple i64 $string))))))"#;
    let json =
        serde_json::to_value(walz::macros::report(source, &Default::default()).unwrap()).unwrap();
    let ty = &json["expansions"][0]["types"]["?T"];
    assert_eq!(ty["kind"], "option");
    assert_eq!(ty["element"]["kind"], "result");
    assert_eq!(ty["element"]["err"]["kind"], "tuple");
    assert_eq!(ty["element"]["err"]["second"]["kind"], "string");
}

#[test]
fn wrong_projection_kind_is_a_diagnostic_and_does_not_run_input() {
    let source = "(module (@macro $m (rule (?x:expr) (infer (?T (option.element ?x))) (i32.const 1))) (func $f (result i32) (@$m (i64.const 0))))";
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_MACRO_TYPE");
    assert!(error.to_string().contains("option.element"));
}

#[test]
fn array_reflection_respects_opaque_module_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("private.walz"),"(module (type $Values (array i64)) (func $new (result (ref $Values)) (array.new_fixed $Values 1 42)) (@export (opaque $Values) (func $new)))").unwrap();
    let path = dir.path().join("main.walz");
    std::fs::write(&path,"(module (@use \"./private.walz\" (type $Values) (func $new)) (@macro $m (rule (?x:expr) (infer (?T (array.element ?x))) (i32.const 1))) (func $f (result i32) (@$m (call $new))))").unwrap();
    let source = walz::source::load(&path).unwrap();
    let error = walz::check(source.text()).unwrap_err();
    assert_eq!(
        error.diagnostic().code,
        "E_OPAQUE_REPRESENTATION",
        "{error}"
    );
}

#[test]
fn every_selector_reifies_the_exact_payload_and_nullable_heap_identity() {
    for (selector, ty, argument, kind, name) in [
        (
            "option.element",
            "(@inst $option i64)",
            "$x",
            "scalar",
            Some("i64"),
        ),
        (
            "list.element",
            "(@inst $list i32)",
            "$x",
            "scalar",
            Some("i32"),
        ),
        (
            "result.ok",
            "(@inst $result i32 i64)",
            "$x",
            "scalar",
            Some("i32"),
        ),
        (
            "result.err",
            "(@inst $result i32 i64)",
            "$x",
            "scalar",
            Some("i64"),
        ),
        (
            "tuple.first",
            "(@tuple i32 i64)",
            "$x",
            "scalar",
            Some("i32"),
        ),
        (
            "tuple.second",
            "(@tuple i32 i64)",
            "$x",
            "scalar",
            Some("i64"),
        ),
        (
            "box.element",
            "(@box i64)",
            "(@move $x)",
            "scalar",
            Some("i64"),
        ),
        (
            "future.element",
            "(@future i32)",
            "(@move $x)",
            "scalar",
            Some("i32"),
        ),
        (
            "stream.element",
            "(@stream i64)",
            "(@move $x)",
            "scalar",
            Some("i64"),
        ),
        (
            "task.element",
            "(@task i32)",
            "(@move $x)",
            "scalar",
            Some("i32"),
        ),
        (
            "array.element",
            "(ref null $Values)",
            "$x",
            "scalar",
            Some("i64"),
        ),
        (
            "ref.heap",
            "(ref null $Values)",
            "$x",
            "nominal",
            Some("$Values"),
        ),
    ] {
        let source = format!(
            r#"(module
          (@use "walz:core" (type $option) (type $result) (type $list))
          (type $Values (array (mut i64)))
          (@macro $q (rule (?x:expr) (infer (?T ({selector} ?x))) ?x))
          (func $f (param $x {ty}) (result {ty}) (@$q {argument})))"#
        );
        let report = walz::macros::report(&source, &Default::default())
            .unwrap_or_else(|e| panic!("{selector}: {e}"));
        let json = serde_json::to_value(report).unwrap();
        let info = &json["expansions"][0]["types"]["?T"];
        assert_eq!(info["kind"], kind, "{selector}");
        if let Some(name) = name {
            assert_eq!(info["name"], name, "{selector}");
        }
    }
}

#[test]
fn a_query_does_not_evaluate_or_retain_the_inputs_effects() {
    let source = r#"(module
      (@macro $ignore (rule (?x:expr) (infer (?T (option.element ?x))) (i32.const 42)))
      (import "host" "danger" (func $danger (result (@option i64))))
      (func $main (export "main") (result i32) (@effect pure) (@$ignore (call $danger))))"#;
    let module = walz::check(source).unwrap();
    let compiled = walz::compile(source).unwrap();
    assert!(module.functions.iter().any(|f| f.name == "$danger"));
    // Querying a signature must not leave the import in executable code.
    assert!(
        !wasmprinter::print_bytes(&compiled.wasm)
            .unwrap()
            .contains("\"danger\"")
    );
}

#[test]
fn generic_opaque_arrays_keep_their_privacy_during_type_queries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("private.walz"),"(module (type $Values (@type.params $T) (array $T)) (func $new (result (ref (@inst $Values i64))) (array.new_fixed (@inst $Values i64) 1 42)) (@export (opaque $Values) (func $new)))").unwrap();
    let path = dir.path().join("main.walz");
    std::fs::write(&path,"(module (@use \"./private.walz\" (func $new)) (@macro $m (rule (?x:expr) (infer (?T (array.element ?x))) (i32.const 1))) (func $f (result i32) (@$m (call $new))))").unwrap();
    let source = walz::source::load(&path).unwrap();
    let error = walz::check(source.text()).unwrap_err();
    assert_eq!(
        error.diagnostic().code,
        "E_OPAQUE_REPRESENTATION",
        "{error}"
    );
}
