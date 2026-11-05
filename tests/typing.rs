use gcwat::ir::{BindingKind, Type};
use gcwat::{check, typing};

const SOURCE: &str = r#"(module
  (type $Option (variant (none) (some i64)))
  (closure $Add (capture $base i64) (param $x i64) (result i64)
    (i64.add (local.get $base) (local.get $x)))
  (func $main (result i64)
    (let $option (variant.new $Option some (i64.const 40))
      (let $add (closure.new $Add (i64.const 2))
        (match $Option (result i64) (local.get $option)
          (case none () (i64.const 0))
          (case some ($value) (closure.call $Add (local.get $add) (local.get $value))))))))"#;

#[test]
fn infers_let_and_pattern_bindings_and_reports_function_boundaries() {
    let module = check(SOURCE).unwrap();
    let report = typing::report(&module);
    let main = report.functions.iter().find(|f| f.name == "$main").unwrap();
    assert_eq!(main.result, "i64");
    for (name, expected) in [
        ("$option", "(ref $Option)"),
        ("$add", "(ref $Add)"),
        ("$value", "i64"),
    ] {
        let local = main.locals.iter().find(|local| local.name == name).unwrap();
        assert_eq!(local.ty, expected);
    }
    assert!(matches!(main.locals[2].kind, BindingKind::Match));
    let closure = report.functions.iter().find(|f| f.name == "$Add").unwrap();
    assert_eq!(closure.locals[closure.params[0].0].name, "$x");
    assert_eq!(closure.locals[closure.captures[0].0].name, "$base");
    assert_eq!(closure.locals[closure.captures[0].0].ty, "i64");
    for (needle, expected) in [
        ("(variant.new", "(ref $Option)"),
        ("(closure.new", "(ref $Add)"),
        ("(match", "i64"),
        ("(local.get $value)", "i64"),
    ] {
        let offset = SOURCE.find(needle).unwrap() + 1;
        let ty = typing::type_at(&module, offset).unwrap();
        assert_eq!(module.type_name(ty), expected);
        assert!(
            main.expressions
                .iter()
                .any(|e| e.span.start <= offset && offset < e.span.end && e.ty == expected)
        );
    }
}

#[test]
fn expression_queries_respect_shadowing_and_utf8_byte_offsets() {
    let source = "(module\n ;; 型の問い合わせは UTF-8 の byte offset\n (func $empty)\n (func $main (result i64) (let $x (i64.const 42) (block (result i64) (drop (let $x (i32.const 1) (local.get $x))) (local.get $x)))))";
    let module = check(source).unwrap();
    for (offset, expected) in [
        (source.find("(local.get $x)").unwrap() + 5, "i32"),
        (source.rfind("(local.get $x)").unwrap() + 5, "i64"),
    ] {
        assert_eq!(
            module.type_name(typing::type_at(&module, offset).unwrap()),
            expected
        );
    }
    assert!(typing::type_at(&module, 0).is_none());
    assert!(typing::type_at(&module, source.find("型").unwrap()).is_none());
    assert!(typing::type_at(&module, source.len()).is_none());
    assert!(typing::type_at(&module, usize::MAX).is_none());
    let report = typing::report(&module);
    assert!(report.functions[0].expressions.is_empty());
}

#[test]
fn type_queries_include_parameter_capture_let_loop_and_case_declarations() {
    let source = r#"(module
      (type $T (variant (some i64)))
      (closure $C (capture $captured i32) (param $argument i64) (result i64)
        (let $reference (variant.new $T some (local.get $argument))
          (loop $walk (bind $state (ref $T) (local.get $reference)) (result i64)
            (match $T (result i64) (local.get $state)
              (case some ($payload) (local.get $payload)))))))"#;
    let module = check(source).unwrap();
    let report = typing::report(&module);
    for (name, expected) in [
        ("$captured", "i32"),
        ("$argument", "i64"),
        ("$reference", "(ref $T)"),
        ("$state", "(ref $T)"),
        ("$payload", "i64"),
    ] {
        let offset = source.find(name).unwrap();
        assert_eq!(
            module.type_name(typing::type_at(&module, offset).unwrap()),
            expected
        );
        let local = report.functions[0]
            .locals
            .iter()
            .find(|local| local.name == name)
            .unwrap();
        assert_eq!(&source[local.span.start..local.span.end], name);
    }
}

#[test]
fn unit_never_and_nullable_references_are_distinct_static_types() {
    let source = "(module (type $T (struct)) (func $unit (nop)) (func $die (result i64) (unreachable)) (func $none (result (ref null $T)) (ref.null $T)))";
    let module = check(source).unwrap();
    for (needle, expected) in [
        ("(nop)", "unit"),
        ("(unreachable)", "never"),
        ("(ref.null", "(ref null $T)"),
    ] {
        let ty = typing::type_at(&module, source.find(needle).unwrap()).unwrap();
        assert_eq!(module.type_name(ty), expected);
    }
    assert_eq!(module.functions[1].body.as_ref().unwrap().ty, Type::Never);
    assert_eq!(module.type_name(&module.functions[1].result), "i64");
}

#[test]
fn optional_let_annotations_are_checked_and_preserved() {
    let source = "(module (type $T (struct)) (func $main (result (ref null $T)) (let $x (ref null $T) (struct.new $T) (local.get $x))))";
    let module = check(source).unwrap();
    let report = typing::report(&module);
    assert_eq!(report.functions[0].locals[0].ty, "(ref null $T)");
    for (needle, expected) in [("(struct.new", "(ref $T)"), ("(local.get", "(ref null $T)")] {
        assert_eq!(
            module.type_name(typing::type_at(&module, source.find(needle).unwrap()).unwrap()),
            expected
        );
    }
    // The declared storage type must also lower to valid Wasm.
    gcwat::compile(source).unwrap();
    let error = check("(module (func $f (result i64) (let $x i64 (i32.const 1) (local.get $x))))")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("type mismatch: expected i64, got i32"),
        "{error}"
    );
    let error = check("(module (type $T (struct)) (func $f (result (ref $T)) (let $x (ref $T) (ref.null $T) (local.get $x))))").unwrap_err().to_string();
    assert!(
        error.contains("expected (ref $T), got (ref null $T)"),
        "{error}"
    );
}

#[test]
fn check_rejects_incompatible_types_before_emission() {
    for (source, expected) in [
        (
            "(module (func $f (result i64) (i64.add (i32.const 1) (i64.const 2))))",
            "expected i64, got i32",
        ),
        (
            "(module (func $f (result i64) (i32.const 1)))",
            "expected i64, got i32",
        ),
        (
            "(module (type $A (struct)) (type $B (struct)) (func $f (param $x (ref $A)) (nop)) (func $g (call $f (struct.new $B))))",
            "expected (ref $A), got (ref $B)",
        ),
        (
            "(module (type $T (struct (field i64))) (func $f (result (ref $T)) (struct.new $T (i32.const 1))))",
            "expected i64, got i32",
        ),
        (
            "(module (closure $C (capture $x i64) (local.get $x)) (func $f (result (ref $C)) (closure.new $C (i32.const 1))))",
            "expected unit, got i64",
        ),
        (
            "(module (closure $C (capture $x i64) (result i64) (local.get $x)) (func $f (result (ref $C)) (closure.new $C (i32.const 1))))",
            "expected i64, got i32",
        ),
        (
            "(module (type $T (variant (a))) (func $f (result i64) (match $T (result i64) (variant.new $T a) (case a () (i32.const 1)))))",
            "expected i64, got i32",
        ),
        (
            "(module (func $f (loop $l (bind $n i64 (i64.const 0)) (recur $l (i32.const 1)))))",
            "expected i64, got i32",
        ),
    ] {
        let error = check(source).unwrap_err().to_string();
        assert!(error.contains(expected), "{source}\n{error}");
        assert!(error.starts_with("1:"), "{error}");
    }
}

#[test]
fn imported_and_forward_function_signatures_are_available_to_checking() {
    let module = check(
        r#"(module
      (import "host" "read" (func $read (param $key i64) (result i32)))
      (func $first (param $n i64) (result i32) (call $second (local.get $n)))
      (func $second (param $n i64) (result i32) (call $read (local.get $n))))"#,
    )
    .unwrap();
    let report = typing::report(&module);
    assert_eq!(report.functions[0].result, "i32");
    assert_eq!(report.functions[0].locals[0].ty, "i64");
    assert!(report.functions[0].expressions.is_empty());
    assert_eq!(report.functions[1].expressions[0].ty, "i32");
}

#[test]
fn suspension_is_conservative_at_unknown_imports_and_propagates() {
    let module = check(
        r#"(module
      (import "host" "operation" (func $host (result i64)))
      (func $pure (result i64) (i64.const 42))
      (func $call (result i64) (call $host))
      (func $forward (result i64) (call $call)))"#,
    )
    .unwrap();
    let analysis = gcwat::analysis::analyze(&module);
    assert!(!analysis.functions[1].effects.may_suspend);
    for index in [0, 2, 3] {
        assert!(analysis.functions[index].effects.may_suspend);
    }
}
