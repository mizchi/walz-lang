#[test]
fn standard_types_and_related_functions_use_wit_style_public_names() {
    walz::check(
        r#"(module
      (@use "walz:core"
        (type $bytes-view) (type $string-view) (type $dict) (type $list-builder)
        (type $string-builder) (type $json-object) (type $json-value)
        (type $string) (type $tuple) (type $box) (type $resource-handle)
        (func $bytes.equal) (func $string-builder.new) (func $json.object-builder.new))
      (func $sample (result (@inst $tuple $string i32))
        (@tuple.new (@string.lit "value") 42))
      (func $main (export "main") (result i64)
        (@let $b (@box.new i64 42)
          (@match (@move $b) (case own ($value) $value)))))"#,
    )
    .unwrap();
    for name in [
        "$bytes-view.View",
        "$string-view.View",
        "$dict.Dict",
        "$list-builder.Builder",
        "$builder.Builder",
        "$json.Object",
        "$json-value.Value",
        "$pair",
        "$own",
        "$resource",
    ] {
        let error =
            walz::check(&format!("(module (@use \"walz:core\" (type {name})))")).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_CORE_MEMBER", "{name}: {error}");
    }
}

#[test]
fn wit_uses_string_and_tuple_while_policy_shapes_keep_their_contracts() {
    let module = walz::check(
        r#"(module
      (@use "walz:core" (type $string) (type $tuple))
      (func $echo (export "echo") (param $value (@inst $tuple $string i64))
        (result (@inst $tuple $string i64)) $value))"#,
    )
    .unwrap();
    let wit = walz::wit::emit(&module).unwrap();
    assert!(wit.contains("tuple<string, s64>"), "{wit}");
    let echo = module.functions.iter().find(|f| f.name == "$echo").unwrap();
    assert_eq!(module.type_name(&echo.result), "(@tuple @string i64)");
}

#[test]
fn historical_imports_and_extension_names_preserve_bindings_and_locations() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("old.walz");
    let old = r#"(module ;; @text.lit $builder.Builder
      (@use "walz:core" (type $text) (type $pair) (type $own)
        (type $builder.Builder) (func $builder.new))
      (func $sample (result (@inst $pair $text i32))
        (@pair.new (@text.lit "old names") 42))
      (func $bad (result i64) (@effects pure)
        (@let $box (@own.new i64 42)
          (@match (@move $box) (case own ($value) $missing)))))"#;
    std::fs::write(&path, old).unwrap();
    let loaded = walz::source::load_legacy(&path).unwrap();
    let error = loaded.diagnostic(walz::check(loaded.text()).unwrap_err());
    assert_eq!(error.diagnostic().code, "E_UNKNOWN_LOCAL", "{error}");
    let label = error.diagnostic().primary.as_ref().unwrap();
    assert_eq!(label.location.line, 8);
    assert_eq!(
        label.location.column,
        old.lines().nth(7).unwrap().find("$missing").unwrap() + 1
    );
    assert!(loaded.text().contains(";; @text.lit $builder.Builder"));
    walz::compile(&loaded.text().replace("$missing", "$value")).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
}

#[test]
fn public_extension_vocabulary_is_strict_and_legacy_types_keep_their_prefixes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("old.walz");
    for (new, old) in [
        (
            "(module (func $f (result @string) (@string.lit \"ok\")))",
            "(module (func $f (result @text) (@text.lit \"ok\")))",
        ),
        (
            "(module (func $f (result (@tuple i32 i32)) (@tuple.new 1 2)))",
            "(module (func $f (result (@pair i32 i32)) (@pair.new 1 2)))",
        ),
        (
            "(module (func $f (result (@box i64)) (@box.new i64 42)))",
            "(module (func $f (result (@own i64)) (@own.new i64 42)))",
        ),
    ] {
        walz::check(new).unwrap();
        assert_eq!(
            walz::check(old).unwrap_err().diagnostic().code,
            "E_WALZ_EXTENSION"
        );
        for source in [old, new] {
            std::fs::write(&path, source).unwrap();
            walz::check(walz::source::load_legacy(&path).unwrap().text()).unwrap();
        }
    }
}

#[test]
fn native_wat_identifiers_are_not_subject_to_library_naming_rules() {
    let source = "(module (type $View (func (result i32))) (func $string-view.View (type $View) i32.const 42) (export \"mixed.Name\" (func $string-view.View)))";
    assert_eq!(walz::compile(source).unwrap().wat, source);
}
