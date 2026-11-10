fn bound_error(source: &str, expected: &str) -> walz::Error {
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_GENERIC_BOUND", "{error}");
    assert!(error.to_string().contains(expected), "{error}");
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|l| l.message.contains("constraint")),
        "{error}"
    );
    error
}

#[test]
fn shared_and_nonunit_bounds_check_even_an_unused_type_argument() {
    let declaration = "(func $accept (@type.params $T) (@where (shared $T) (nonunit $T)) (nop))";
    for argument in ["i64", "@string", "(@option i64)", "(ref $Cell)", "$Cell"] {
        let source = format!(
            "(module (type $Cell (struct (field (mut i64)))) {declaration} (func $f (call (@inst $accept {argument}))))"
        );
        walz::compile(&source).unwrap();
    }
    for argument in [
        "(@box i64)",
        "(@tuple i32 (@future i64))",
        "(@stream @string)",
        "(@list (@box i64))",
    ] {
        bound_error(
            &format!("(module {declaration} (func $f (call (@inst $accept {argument}))))"),
            "shared",
        );
    }
    bound_error(
        &format!("(module {declaration} (func $f (call (@inst $accept @unit))))"),
        "nonunit",
    );
}

#[test]
fn callable_checks_nominal_closure_signature_before_the_template_body() {
    let declaration = r#"(func $apply (@type.params $T $U $F)
      (@where (shared $T $U) (nonunit $T $U)
        (callable $F (param $T) (result $U)))
      (param $f (ref $F)) (param $x $T) (result $U)
      (@closure.call $F (local.get $f) (local.get $x)))"#;
    let source = format!(
        r#"(module
      (@closure $Id (param $x i64) (result i64) (local.get $x))
      {declaration}
      (func $main (export "main") (result i64) (call (@inst $apply i64 i64 $Id) (@closure.new $Id) (i64.const 42))))"#
    );
    walz::compile(&source).unwrap();
    bound_error(
        &source.replace(
            "(@inst $apply i64 i64 $Id)",
            "(@inst $apply i64 @string $Id)",
        ),
        "signature",
    );
    let source = format!(
        "(module (type $Cell (struct (field i64))) {declaration} (func $f (result i64) (call (@inst $apply i64 i64 $Cell) (struct.new $Cell (i64.const 1)) (i64.const 2))))"
    );
    bound_error(&source, "closure");
}

#[test]
fn inferred_effects_are_checked_even_when_the_callback_is_never_called() {
    let template = r#"(func $require (@type.params $F)
      (@where (callable $F (param i64) (result i64) (@effect pure))) (nop))"#;
    let source = format!(
        r#"(module
      (type $Cell (struct (field (mut i64))))
      (@closure $Set (@capture $cell (ref $Cell)) (param $x i64) (result i64)
        (block (result i64) (struct.set $Cell 0 (local.get $cell) (local.get $x)) (local.get $x)))
      {template} (func $unused (call (@inst $require $Set))) (func $main (export "main") (nop)))"#
    );
    bound_error(&source, "writes");
    walz::compile(&source.replace("(@effect pure)", "(@effect writes)")).unwrap();
}

#[test]
fn constraints_apply_to_types_aliases_and_generic_closures() {
    let source = r#"(module
      (@type.alias $Numbers (@type.params $T) (@where (shared $T) (nonunit $T)) (@list $T))
      (type $Box (@type.params $T) (@where (shared $T) (nonunit $T)) (struct (field $T)))
      (@closure $Id (@type.params $T) (@where (shared $T) (nonunit $T)) (param $x $T) (result $T) (local.get $x))
      (func $main (export "main") (result i64)
        (@let $box (struct.new (@inst $Box i64) (i64.const 42))
          (@closure.call (@inst $Id i64) (@closure.new (@inst $Id i64)) (struct.get (@inst $Box i64) 0 (local.get $box)))))
      (func $empty (result (@inst $Numbers i64)) (@list.new i64)))"#;
    walz::compile(source).unwrap();
    bound_error(
        "(module (@type.alias $Unused (@type.params $T) (@where (shared $T)) i64) (@type.alias $Bad (@inst $Unused (@box i64))))",
        "shared",
    );
    bound_error(
        "(module (type $Unused (@type.params $T) (@where (shared $T)) (struct)) (func $f (result (ref (@inst $Unused (@box i64)))) (struct.new (@inst $Unused (@box i64)))))",
        "shared",
    );
}

#[test]
fn malformed_constraints_are_rejected_without_instantiation() {
    for clause in [
        "(@where)",
        "(@where (shared))",
        "(@where (shared $Missing))",
        "(@where (shared i64))",
        "(@where (nonunit $Missing))",
        "(@where (unknown $T))",
        "(@where (callable $F (param i64)))",
        "(@where (callable $F (param) (result)))",
        "(@where (callable $F (param) (result @unit) (@effect pure writes)))",
        "(@where (callable $F (param) (result @unit) (@effect imaginary)))",
    ] {
        let source = format!("(module (func $f (@type.params $T $F) {clause} (nop)))");
        assert!(walz::check(&source).is_err(), "accepted {source}");
    }
    for source in [
        "(module (func $f (@where (shared $T)) (nop)))",
        "(module (func $f (@type.params $T) (param $x $T) (@where (shared $T)) (nop)))",
        "(module (func $f (@type.params $T) (@where (shared $T)) (@where (nonunit $T)) (nop)))",
    ] {
        assert!(walz::check(source).is_err(), "accepted {source}");
    }
}

#[test]
fn owned_closures_and_borrows_cannot_satisfy_shared_or_callable_bounds() {
    let source = r#"(module
      (@closure $Once (@capture $owner (@box i64)) (result i64)
        (@match (@box i64) (result i64) (@move $owner) (case own ($x) (local.get $x))))
      (func $require (@type.params $F) (@where (callable $F (param) (result i64))) (nop))
      (func $f (call (@inst $require $Once))))"#;
    bound_error(source, "reusable");
    let source = r#"(module
      (type $File (@resource "file" $close))
      (import "file" "close" (func $close (param $id i32)))
      (func $require (@type.params $T) (@where (shared $T)) (nop))
      (func $f (call (@inst $require (@borrow $File)))))"#;
    bound_error(source, "shared");
}

#[test]
fn callback_effect_bounds_follow_calls_and_preserve_caught_exception_semantics() {
    let source = r#"(module
      (tag $Failure (param i32))
      (func $raise (throw $Failure (i32.const 1)))
      (@closure $Throw (call $raise))
      (func $require (@type.params $F)
        (@where (callable $F (param) (result @unit) (@effect pure))) (nop))
      (func $f (call (@inst $require $Throw))))"#;
    let error = bound_error(source, "throws");
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|l| l.message.contains("call to `$raise`")),
        "{error}"
    );
    walz::compile(&source.replace("(@effect pure)", "(@effect (throws $Failure))")).unwrap();
    walz::compile(&source.replace("(@effect pure)", "(@effect (throws *))")).unwrap();
    let caught = source.replace(
        "(@closure $Throw (call $raise))",
        "(@closure $Throw (@try (do (call $raise)) (catch $Failure ($error) (nop))))",
    );
    walz::compile(&caught).unwrap();
}

#[test]
fn callback_effect_bounds_use_trusted_import_policy_and_never_trust_a_pure_annotation() {
    let source = r#"(module
      (import "clock" "now" (func $now (result i64)))
      (@closure $Now (result i64) (call $now))
      (func $require (@type.params $F)
        (@where (callable $F (param) (result i64) (@effect host))) (nop))
      (func $f (call (@inst $require $Now))))"#;
    bound_error(source, "exceed");
    let policy = walz::bindings::BindingPolicy::parse(r#"{"imports":[{"module":"clock","name":"now","params":[],"result":"i64","suspends":false,"capability":"clock"}]}"#).unwrap();
    walz::check_with_policy(source, &policy).unwrap();
    let lying = source
        .replace(
            "(@closure $Now (result i64)",
            "(@closure $Now (result i64) (@effect pure)",
        )
        .replace("(@effect host)", "(@effect pure)");
    let error = walz::check_with_policy(&lying, &policy).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_GENERIC_BOUND", "{error}");
    assert!(error.to_string().contains("host"));
}

#[test]
fn imported_constraints_use_concrete_aliases_and_report_both_source_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("api.walz"), "(module\n (@export (func $require))\n (func $require (@type.params $T) (@where (shared $T)) (nop)))").unwrap();
    let main = dir.path().join("main.walz");
    std::fs::write(&main, "(module\n (@use \"./api.walz\" (func $require))\n (@type.alias $Count i64)\n (func $main (call (@inst $require $Count))))").unwrap();
    let loaded = walz::source::load(&main).unwrap();
    walz::compile(loaded.text()).unwrap();
    std::fs::write(&main, "(module\n (@use \"./api.walz\" (func $require))\n (func $main (call (@inst $require (@box i64)))))").unwrap();
    let loaded = walz::source::load(&main).unwrap();
    let error = loaded.diagnostic(walz::check(loaded.text()).unwrap_err());
    assert_eq!(error.diagnostic().code, "E_GENERIC_BOUND");
    assert!(
        error
            .diagnostic()
            .primary
            .as_ref()
            .unwrap()
            .location
            .path
            .as_ref()
            .unwrap()
            .ends_with("main.walz")
    );
    assert!(error.diagnostic().related.iter().any(|l| {
        l.location
            .path
            .as_ref()
            .is_some_and(|p| p.ends_with("api.walz"))
    }));
}

#[test]
fn constraint_only_types_do_not_create_runtime_functions_or_abi_requirements() {
    let bounded = "(module (func $require (@type.params $T) (@where (shared $T)) (nop)) (func $main (export \"main\") (call (@inst $require (@list (@tuple i64 @string))))))";
    let plain = bounded.replace("(@where (shared $T))", "");
    let a = walz::compile(bounded).unwrap();
    let b = walz::compile(&plain).unwrap();
    assert_eq!(a.wat, b.wat);
    assert_eq!(a.wasm, b.wasm);
}

#[test]
fn shared_checks_recursive_gc_data_without_hiding_borrowed_fields() {
    let valid = "(module (type $Node (struct (field (mut i64)) (field (ref null $Node)))) (func $require (@type.params $T) (@where (shared $T)) (nop)) (func $f (call (@inst $require (ref $Node)))))";
    walz::compile(valid).unwrap();
    let invalid = r#"(module
      (type $File (@resource "file" $close))
      (import "file" "close" (func $close (param $id i32)))
      (type $Cell (struct (field (@borrow $File))))
      (func $require (@type.params $T) (@where (shared $T)) (nop))
      (func $f (call (@inst $require (ref $Cell)))))"#;
    bound_error(invalid, "shared");
}

#[test]
fn unit_and_non_nominal_callbacks_fail_at_application_before_invalid_headers() {
    bound_error(
        "(module (func $id (@type.params $T) (@where (nonunit $T)) (param $x $T) (result $T) (local.get $x)) (func $main (call (@inst $id @unit) (nop))))",
        "nonunit",
    );
    bound_error(
        "(module (func $apply (@type.params $F) (@where (callable $F (param) (result @unit))) (param $f (ref $F)) (@closure.call $F (local.get $f))) (func $main (call (@inst $apply i64) (i64.const 0))))",
        "closure",
    );
}
