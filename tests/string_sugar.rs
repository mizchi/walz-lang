#[test]
fn expected_string_literals_match_explicit_wasm_and_portable_boundaries() {
    let short = r#"(module (@use "walz:core" (type $string))
      (func $id (param $s $string) (result $string) $s)
      (func $main (export "main") (result $string) (@effect pure)
        (@let $s (type $string) "日本😀"
          (if (result $string) 1 (then (call $id $s)) (else "unused")))))"#;
    let explicit = short
        .replace("\"日本😀\"", "(@string.lit \"日本😀\")")
        .replace("\"unused\"", "(@string.lit \"unused\")");
    for optimize in [false, true] {
        let options = walz::CompileOptions { optimize };
        assert_eq!(
            walz::compile_with_options(short, options).unwrap().wasm,
            walz::compile_with_options(&explicit, options).unwrap().wasm
        );
    }
    walz::component::compile(short).unwrap();
    walz::component::core_js::compile(short).unwrap();
    let direct = r#"(module (@use "walz:core" (type $string)) (func $main (export "main") (result $string) "direct"))"#;
    walz::check(direct).unwrap();
}
#[test]
fn string_literals_need_an_expected_string_type_and_diagnostics_use_surface_names() {
    for body in [
        "(func $f (@let $x \"x\" (nop)))",
        "(func $f (result i32) \"x\")",
        "(func $f (result $string) \"\\ff\")",
    ] {
        let source = format!("(module (@use \"walz:core\" (type $string)) {body})");
        let error = walz::check(&source).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_LITERAL_TYPE", "{error}");
        assert!(!error.diagnostic().help.is_empty());
    }
    let error=walz::check("(module (@use \"walz:core\" (type $string)) (func $f (param $s $string) (result i32) (@string.len $s)))").unwrap_err();
    assert!(
        error.diagnostic().message.contains("@string.len"),
        "{error}"
    );
    assert!(!error.diagnostic().message.contains("text.len"));
    assert!(
        error
            .diagnostic()
            .help
            .iter()
            .any(|s| s.contains("@string.byte-len")),
        "{error}"
    );
}
