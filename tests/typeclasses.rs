use std::process::Command;

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compilation =
            wlt::compile_with_options(source, wlt::CompileOptions { optimize }).unwrap();
        assert!(!compilation.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.wasm");
        std::fs::write(&path, compilation.wasm).unwrap();
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                "main",
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

fn error(source: &str, code: &str, message: &str) {
    let error = wlt::compile(source).err().expect("invalid source accepted");
    assert_eq!(error.diagnostic().code, code, "{source}: {error}");
    assert!(error.to_string().contains(message), "{source}: {error}");
    assert!(error.diagnostic().primary.is_some());
}

#[test]
fn add_instances_use_the_expected_scalar_type_and_emit_plain_numeric_instructions() {
    for (ty, a, b, expected) in [
        ("i32", "1", "3", "4"),
        ("i64", "40", "2", "42"),
        ("f32", "1.5", "40.5", "42"),
        ("f64", "1.5", "40.5", "42"),
        ("i32", "2147483647", "1", "-2147483648"),
        ("i64", "0xffff_ffff_ffff_ffff", "1", "0"),
    ] {
        let source = format!(
            "(module (func $main (export \"main\") (result {ty}) (%effect pure) (+ {a} {b})))"
        );
        let explicit = source.replace(
            &format!("(+ {a} {b})"),
            &format!("({ty}.add ({ty}.const {a}) ({ty}.const {b}))"),
        );
        for optimize in [false, true] {
            let options = wlt::CompileOptions { optimize };
            assert_eq!(
                wlt::compile_with_options(&source, options).unwrap().wasm,
                wlt::compile_with_options(&explicit, options).unwrap().wasm
            );
        }
        run(&source, expected);
    }
}

#[test]
fn either_operand_can_supply_the_type_without_defaulting_numeric_literals() {
    run(
        r#"(module (func $main (export "main") (result i64)
      (%let (($x (type i64) 40) ($y (+ 1 $x)) ($z (+ $y 1))) $z)))"#,
        "42",
    );
    run(
        r#"(module (func $main (export "main") (result i32)
      (= (+ 1 2) (+ 1 (i64.const 2)))))"#,
        "1",
    );
    for source in [
        "(module (func $f (result i32) (%let $x (+ 1 3) $x)))",
        "(module (func $f (result i32) (= 1 3)))",
        "(module (func $f (result i32) (< 1 3)))",
    ] {
        error(source, "E_OPERATOR_TYPE", "operand type");
    }
}

#[test]
fn integer_eq_and_ord_keep_operand_width_and_signed_order() {
    for ty in ["i32", "i64"] {
        for (symbol, instruction, a, b, expected) in [
            ("=", "eq", "-1", "0xffff_ffff", "1"),
            ("!=", "ne", "-1", "0", "1"),
            ("<", "lt_s", "-1", "0", "1"),
            ("<=", "le_s", "-1", "-1", "1"),
            (">", "gt_s", "0", "-1", "1"),
            (">=", "ge_s", "0", "0", "1"),
        ] {
            let b = if ty == "i64" && b == "0xffff_ffff" {
                "0xffff_ffff_ffff_ffff"
            } else {
                b
            };
            let source = format!(
                r#"(module
              (func $compare (param $a {ty}) (param $b {ty}) (result i32) (%effect pure) ({symbol} $a $b))
              (func $main (export "main") (result i32) (%effect pure) (call $compare {a} {b})))"#
            );
            let explicit = source.replace(
                &format!("({symbol} $a $b)"),
                &format!("({ty}.{instruction} $a $b)"),
            );
            for optimize in [false, true] {
                let options = wlt::CompileOptions { optimize };
                assert_eq!(
                    wlt::compile_with_options(&source, options).unwrap().wasm,
                    wlt::compile_with_options(&explicit, options).unwrap().wasm
                );
            }
            run(&source, expected);
        }
    }
}

#[test]
fn generic_add_eq_and_ord_constraints_resolve_after_monomorphization() {
    run(
        r#"(module
      (%namespace $n
        (%public (func $choose))
        (func $choose (%type.params $T) (%where (Add $T) (Eq $T) (Ord $T))
          (param $a $T) (param $b $T) (result $T) (%effect pure)
          (if (result $T) (< $a $b) (then (+ $a $b)) (else $a))))
      (%type.alias $Count i64)
      (func $main (export "main") (result i64) (%effect pure)
        (call (%inst $n.choose $Count) 2 40)))"#,
        "42",
    );
    run(
        r#"(module
      (func $sum (%type.params $T) (%where (Add $T)) (param $x $T) (param $y $T) (result $T) (+ $x $y))
      (func $main (export "main") (result f64) (call (%inst $sum f64) 1.5 40.5)))"#,
        "42",
    );
}

#[test]
fn unavailable_class_instances_are_rejected_before_dead_code_elimination() {
    for class in ["Add", "Eq", "Ord"] {
        for argument in ["v128", "(ref $S)", "$N", "(%own i64)", "%text", "%unit"] {
            let source = format!(
                "(module (type $S (struct)) (type $N (%newtype i64))
              (func $require (%type.params $T) (%where ({class} $T)) (nop))
              (func $unused (call (%inst $require {argument}))))"
            );
            let error = wlt::check(&source).unwrap_err();
            assert_eq!(error.diagnostic().code, "E_GENERIC_BOUND", "{error}");
            assert!(error.to_string().contains(class), "{error}");
            assert!(error.to_string().contains("instance"), "{error}");
            assert!(!error.diagnostic().related.is_empty());
        }
    }
    for class in ["Eq", "Ord"] {
        for ty in ["f32", "f64"] {
            error(
                &format!(
                    "(module (func $require (%type.params $T) (%where ({class} $T)) (nop)) (func $unused (call (%inst $require {ty}))))"
                ),
                "E_GENERIC_BOUND",
                "instance",
            );
        }
    }
    error(
        "(module (func $f (%type.params $T) (%where (Add $T)) (param $x $T) $x) (func $g (call (%inst $f %unit))))",
        "E_GENERIC_BOUND",
        "instance",
    );
}

#[test]
fn class_bounds_apply_to_types_aliases_and_closures_even_without_operator_calls() {
    for declaration in [
        "(type $Box (%type.params $T) (%where (Add $T)) (struct))",
        "(%type.alias $Box (%type.params $T) (%where (Eq $T)) i64)",
        "(%closure $Box (%type.params $T) (%where (Ord $T)) (nop))",
    ] {
        let application = if declaration.starts_with("(%type.alias") {
            "(func $f (param $x (%inst $Box f64)) (nop))"
        } else {
            "(func $f (param $x (ref (%inst $Box f64))) (nop))"
        };
        let source = format!("(module {declaration} {application})");
        if declaration.contains("(Add") {
            wlt::check(&source).unwrap();
        } else {
            error(&source, "E_GENERIC_BOUND", "instance");
        }
    }
}

#[test]
fn malformed_class_constraints_are_checked_without_instantiation() {
    for clause in [
        "(Add)",
        "(Eq i32)",
        "(Ord $Missing)",
        "(Add (ref $T))",
        "(Unknown $T)",
    ] {
        error(
            &format!("(module (func $f (%type.params $T) (%where {clause}) (nop)))"),
            "E_GENERIC_CONSTRAINT",
            "constraint",
        );
    }
}

#[test]
fn operators_reject_mixed_types_unsupported_instances_and_bad_arity() {
    for source in [
        "(module (func $f (result i64) (+ (i32.const 1) (i64.const 2))))",
        "(module (func $f (param $x i64) (result i32) (< $x (i32.const 1))))",
    ] {
        error(source, "E_TYPE_MISMATCH", "type mismatch");
    }
    for source in [
        "(module (func $f (param $x f32) (result i32) (= $x $x)))",
        "(module (func $f (param $x f64) (result i32) (< $x $x)))",
        "(module (type $N (%newtype i32)) (func $f (param $x $N) (result $N) (+ $x $x)))",
        "(module (func $f (param $x v128) (result v128) (+ $x $x)))",
        "(module (type $S (struct)) (func $f (param $x (ref $S)) (result i32) (= $x $x)))",
    ] {
        error(source, "E_CLASS_INSTANCE", "instance");
    }
    for expression in ["(+)", "(+ 1)", "(+ 1 2 3)", "(=)", "(< 1)"] {
        error(
            &format!("(module (func $f (result i32) {expression}))"),
            "E_OPERATOR_ARITY",
            "two operands",
        );
    }
}

#[test]
fn deferred_operand_typing_preserves_local_hygiene_source_order_and_single_evaluation() {
    run(
        r#"(module
      (type $Trace (struct (field $log (mut i32))))
      (func $mark (param $t (ref $Trace)) (param $n i32) (result i32)
        (block (result i32)
          (struct.set $Trace $log $t (+ (i32.mul (struct.get $Trace $log $t) 10) $n)) $n))
      (func $main (export "main") (result i64)
        (%let $t (struct.new $Trace 0)
          (%let $sum
            (+ (%let $hidden (type i32) (call $mark $t 1) (+ 0 0))
               (%let $witness (type i64) (i64.extend_i32_s (call $mark $t 2)) $witness))
            (+ (i64.mul (i64.extend_i32_s (struct.get $Trace $log $t)) 100) $sum)))))"#,
        "1202",
    );
}

#[test]
fn abrupt_operands_and_owned_values_retain_their_existing_static_contracts() {
    run(
        r#"(module (tag $Stop)
      (func $main (export "main") (result i32)
        (%try (result i32) (do (= (throw $Stop) (i64.const 1))) (catch $Stop () 42))))"#,
        "42",
    );
    error(
        r#"(module
      (import "host" "f" (func $host (result i32)))
      (func $f (result i32) (%effect pure) (+ (call $host) 1)))"#,
        "E_EFFECT_CONTRACT",
        "effect contract",
    );
    let source = "(module (func $f (%let $x (%own.new i64 1) (drop (= $x $x)))))";
    assert!(wlt::compile(source).is_err());
}

#[test]
fn nested_unannotated_operators_reuse_typed_operands_with_bounded_work() {
    let mut value = "$x".to_owned();
    for _ in 0..80 {
        value = format!("(+ 1 {value})");
    }
    run(
        &format!(
            "(module (func $main (export \"main\") (result i64) (%let $x (type i64) 40 (%let $answer {value} $answer))))"
        ),
        "120",
    );
}

#[test]
fn formatting_and_native_wat_keep_their_original_tokens() {
    let source =
        "(module (func $main (export \"main\") (result i32) (%let $x (type i32) 40 (+ $x 2))))";
    let formatted = wlt::format::source(source).unwrap();
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
    let native =
        r#"(module (memory 1) (data (i32.const 0) "(+ 1 3)") (func (result i32) i32.const 4))"#;
    assert_eq!(wlt::compile(native).unwrap().wat, native);
}

#[test]
fn inspection_distinguishes_comparison_results_from_their_operands() {
    let source = "(module (func $f (param $x i64) (result i32) (< $x 1)))";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    std::fs::write(&path, source).unwrap();
    let loaded = wlt::source::load(path).unwrap();
    for (needle, expected) in [("<", "i32"), ("$x 1", "i64"), ("1)))", "i64")] {
        let query = wlt::inspect::type_at(
            &loaded,
            None,
            1,
            source.find(needle).unwrap() + 1,
            &wlt::bindings::BindingPolicy::default(),
        )
        .unwrap();
        assert_eq!(query.mode, "typed");
        assert_eq!(query.matches[0].ty, expected, "{query:?}");
    }
}

#[test]
fn class_operators_use_the_existing_law_and_ownership_verifier() {
    let source = r#"(module
      (%law "add identity" (param $x i32) (= (+ $x 0) $x))
      (func $main (export "main") (result i32) (%effect pure) (+ 40 2)))"#;
    wlt::verified::compile(source).unwrap();
    assert!(wlt::verified::compile(&source.replace("(+ $x 0)", "(+ $x 1)")).is_err());
}
