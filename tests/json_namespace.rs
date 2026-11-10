#[test]
fn json_construction_parsing_and_object_writing_have_distinct_public_names() {
    walz::check(
        r#"(module
      (@use "walz:core" (type $json-value) (type $json-kind) (type $json-object)
        (type $string) (type $result)
        (func $json.from-string) (func $json.parse) (func $json.stringify)
        (func $json.kind) (func $json.null) (func $json.boolean) (func $json.number)
        (func $json.array) (func $json.object) (func $json.prepend) (func $json.set)
        (func $json.get) (func $json.at) (func $json.as-string)
        (func $json.as-number) (func $json.as-boolean)
        (func $json.object-builder.new) (func $json.object-builder.string)
        (func $json.object-builder.finish))
      (func $quote (export "quote") (param $text $string) (result $string)
        (call $json.stringify (call $json.from-string $text)))
      (func $parse (param $text $string) (result (@inst $result (ref $json-value) i32))
        (call $json.parse $text)))"#,
    )
    .unwrap();
    for name in [
        "$string.json",
        "$string-builder.json",
        "$json-value.string",
        "$json-value.parse",
        "$json-value.stringify",
        "$json-object.new",
        "$json.quote-string",
        "$json-string.quote",
    ] {
        let error =
            walz::check(&format!("(module (@use \"walz:core\" (func {name})))")).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_CORE_MEMBER", "{name}: {error}");
    }
}

#[test]
fn plain_strings_and_builders_do_not_depend_on_json_and_construction_is_selective() {
    for source in [
        r#"(module (@use "walz:core" (type $string) (func $string.concat))
          (func $main (export "main") (param $s $string) (result $string) (call $string.concat $s $s)))"#,
        r#"(module (@use "walz:core" (type $string) (func $string-builder.new) (func $string-builder.finish))
          (func $main (export "main") (result $string) (call $string-builder.finish (call $string-builder.new))))"#,
    ] {
        let module = walz::check(source).unwrap();
        assert!(module.functions.iter().all(|f| !f.name.contains("json")));
    }
    let built = walz::compile(
        r#"(module
      (@use "walz:core" (type $string) (type $json-value) (func $json.from-string))
      (func $make (export "make") (param $text $string) (result (ref $json-value))
        (call $json.from-string $text)))"#,
    )
    .unwrap();
    for unused in [
        ".json.parse",
        ".json.stringify",
        ".json-string.quote",
        ".string-builder",
    ] {
        assert!(
            !built.wat.contains(unused),
            "kept unused dependency {unused}"
        );
    }
}

#[test]
fn frozen_import_adaptation_preserves_aliases_and_ignores_comments_and_strings() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.walz");
    for source in [
        r#"(module ;; $json.new $json.Object
          (@use "walz:core" (type $json-value) (type $string)
            (func $json.object) (func $json.null) (func $json.stringify))
          (func $object (result (ref $json-value)) (call $json.object))
          (func $null (result (ref $json-value)) (call $json.null))
          (func $text (result $string) (@string.lit "$json.new $json.Object")))"#,
        r#"(module (@use "walz:core" (type $json-value.Value) (type $text)
            (func $json-value.string) (func $json-value.stringify))
          (func $text (result $text)
            (call $json-value.stringify (call $json-value.string (@text.lit "null")))))"#,
        r#"(module (@use "walz:core" (type $json.Object) (type $text)
            (func $json.new) (func $json.null) (func $json.finish))
          (func $text (result $text)
            (@let $out (call $json.new)
              (block (result $text) (call $json.null $out (@text.lit "key")) (call $json.finish $out)))))"#,
    ] {
        std::fs::write(&path, source).unwrap();
        let loaded = walz::source::load_legacy(&path).unwrap();
        walz::compile(loaded.text()).unwrap_or_else(|e| panic!("{source}\n{e}"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}
