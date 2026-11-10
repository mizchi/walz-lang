#[test]
fn at_extensions_and_user_macros_produce_checked_ir() {
    let source = r#"(module
      (@use "walz:core" (type $option))
      (@macro $twice (rule (?value:expr) (@let $saved ?value (+ $saved $saved))))
      (func $main (export "main") (result i64) (@effect pure)
        (@match (@option.some i64 (@$twice (i64.const 21)))
          (case some ($value) $value) (case none () 0))))"#;
    let compiled = walz::compile(source).unwrap();
    assert!(compiled.ir.is_some());
    assert!(!compiled.analysis.opaque);
}

#[test]
fn at_effects_cannot_be_discarded_as_wat_annotations() {
    let source = r#"(module
      (import "host" "write" (func $write))
      (func $main (@effect pure) (call $write)))"#;
    let error = walz::compile(source)
        .err()
        .expect("effect contract was ignored");
    assert_eq!(error.diagnostic().code, "E_EFFECT_CONTRACT");
    let source = "(module (@test \"invalid\" (i64.const 1)))";
    assert!(
        walz::compile(source).is_err(),
        "test declaration was ignored"
    );
    assert!(
        walz::compile("(module (@let {}))").is_err(),
        "malformed extension was ignored"
    );
}

#[test]
fn native_annotations_strings_comments_and_identifiers_keep_their_bytes() {
    let source = r#"(module
      (@custom "prefix" "%let @let")
      (@metadata (@macro $ignored (rule () (@let $x 0 $x))))
      ;; @let and %let here are comments.
      (func $id%let@let (export "%let @let") (result i32) i32.const 42))"#;
    assert_eq!(walz::compile(source).unwrap().wat, source);
}

#[test]
fn percent_extensions_are_rejected_and_missing_prefix_help_uses_at() {
    assert!(walz::compile("(module (%let $x i32 1 $x))").is_err());
    let error = walz::check("(module (func $main (let $x (i32.const 1) (drop $x))))").unwrap_err();
    assert_eq!(error.diagnostic().code, "E_WALZ_PREFIX");
    assert!(error.to_string().contains("@let"), "{error}");
}
