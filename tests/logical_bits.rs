use std::process::Command;

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compiled = wlt::compile_with_options(source, wlt::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
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

fn error(source: &str, code: &str) {
    let error = wlt::compile(source).err().expect("invalid source accepted");
    assert_eq!(error.diagnostic().code, code, "{source}: {error}");
    assert!(error.diagnostic().primary.is_some());
}

#[test]
fn logical_values_use_i32_truth_and_return_canonical_zero_or_one() {
    for (op, a, b, expected) in [
        ("&&", 0, 7, "0"),
        ("&&", 2, 4, "1"),
        ("&&", -1, 0, "0"),
        ("&&", i32::MIN, -7, "1"),
        ("||", 0, 0, "0"),
        ("||", 0, -3, "1"),
        ("||", 2, 4, "1"),
        ("||", i32::MIN, 0, "1"),
    ] {
        run(
            &format!(
                "(module (func $main (export \"main\") (result i32) (%let $r ({op} {a} {b}) $r)))"
            ),
            expected,
        );
    }
}

#[test]
fn logical_operators_emit_the_same_wasm_as_explicit_short_circuit_if() {
    for (op, then_body, else_body) in [("&&", "(i32.ne $b 0)", "0"), ("||", "1", "(i32.ne $b 0)")] {
        let source = format!(
            "(module (func $f (export \"f\") (param $a i32) (param $b i32) (result i32) (%effect pure) ({op} $a $b)))"
        );
        let explicit = source.replace(
            &format!("({op} $a $b)"),
            &format!("(if (result i32) $a (then {then_body}) (else {else_body}))"),
        );
        for optimize in [false, true] {
            let options = wlt::CompileOptions { optimize };
            assert_eq!(
                wlt::compile_with_options(&source, options).unwrap().wasm,
                wlt::compile_with_options(&explicit, options).unwrap().wasm
            );
        }
    }
}

#[test]
fn short_circuit_and_eager_bits_evaluate_their_reached_operands_once_in_source_order() {
    for (op, a, b, expected) in [
        ("&&", 0, 7, "10"),
        ("||", -1, 7, "11"),
        ("&&", 2, 7, "121"),
        ("||", 0, 0, "120"),
        ("&", 0, 7, "120"),
        ("|", 2, 4, "126"),
        ("^", 2, 4, "126"),
    ] {
        run(
            &format!(
                r#"(module
          (type $Trace (struct (field $log (mut i32))))
          (func $mark (param $t (ref $Trace)) (param $n i32) (param $value i32) (result i32)
            (block (result i32)
              (struct.set $Trace $log $t (+ (i32.mul (struct.get $Trace $log $t) 10) $n)) $value))
          (func $main (export "main") (result i32)
            (%let (($t (struct.new $Trace 0))
                   ($r ({op} (call $mark $t 1 {a}) (call $mark $t 2 {b}))))
              (+ (i32.mul (struct.get $Trace $log $t) 10) $r))))"#
            ),
            expected,
        );
    }
}

#[test]
fn skipped_rhs_never_traps_throws_or_returns() {
    for (body, expected) in [
        ("(&& 0 (i32.div_s 1 0))", "0"),
        ("(|| -3 (unreachable))", "1"),
        ("(&& 0 (throw $Stop))", "0"),
        ("(|| 1 (return 42))", "1"),
        ("(&& 1 (return 42))", "42"),
        (
            "(%try (result i32) (do (|| 0 (throw $Stop))) (catch $Stop () 42))",
            "42",
        ),
        ("(i32.add (&& (return 42) (unreachable)) 1)", "42"),
    ] {
        run(
            &format!("(module (tag $Stop) (func $main (export \"main\") (result i32) {body}))"),
            expected,
        );
    }
}

#[test]
fn skipped_rhs_still_obeys_type_and_effect_contracts() {
    for body in ["(&& 0 (call $host))", "(|| 1 (call $host))"] {
        error(
            &format!(
                r#"(module (import "host" "f" (func $host (result i32)))
          (func $f (result i32) (%effect pure) {body}))"#
            ),
            "E_EFFECT_CONTRACT",
        );
    }
    for body in [
        "(&& 0 (i64.const 1))",
        "(|| 1 (f32.const 1))",
        "(&& (i64.const 1) 0)",
    ] {
        error(
            &format!("(module (func $f (result i32) {body}))"),
            "E_TYPE_MISMATCH",
        );
    }
    error(
        "(module (func $f (result i64) (&& 1 2)))",
        "E_TYPE_MISMATCH",
    );
}

#[test]
fn bitwise_and_shift_instances_emit_native_integer_operations() {
    for ty in ["i32", "i64"] {
        for (symbol, instruction) in [
            ("&", "and"),
            ("|", "or"),
            ("^", "xor"),
            ("<<", "shl"),
            (">>", "shr_s"),
            (">>>", "shr_u"),
        ] {
            let source = format!(
                "(module (func $f (export \"f\") (param $a {ty}) (param $b {ty}) (result {ty}) (%effect pure) ({symbol} $a $b)))"
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
        }
    }
    for (ty, expression, expected) in [
        ("i32", "(& -1 42)", "42"),
        ("i64", "(| 40 2)", "42"),
        ("i32", "(^ 46 4)", "42"),
        ("i32", "(<< 1 32)", "1"),
        ("i64", "(<< 1 64)", "1"),
        ("i32", "(<< 1 -1)", "-2147483648"),
        ("i64", "(<< 1 -1)", "-9223372036854775808"),
        ("i32", "(>> -1 1)", "-1"),
        ("i64", "(>> -1 65)", "-1"),
        ("i32", "(>>> -1 1)", "2147483647"),
        ("i64", "(>>> -1 1)", "9223372036854775807"),
        ("i32", "(>>> -1 0)", "-1"),
        ("i64", "(>>> -1 0)", "-1"),
        ("i32", "(>>> -2147483648 31)", "1"),
        ("i64", "(>>> -9223372036854775808 63)", "1"),
        ("i32", "(<< 1 33)", "2"),
    ] {
        run(
            &format!("(module (func $main (export \"main\") (result {ty}) {expression}))"),
            expected,
        );
    }
}

#[test]
fn logical_bitwise_and_shift_bounds_resolve_in_generic_functions() {
    run(
        r#"(module
      (func $both (%type.params $T) (%where (Logical $T))
        (param $a $T) (param $b $T) (result $T) (&& $a $b))
      (func $bits (%type.params $T) (%where (Bitwise $T) (Shift $T))
        (param $x $T) (param $mask $T) (param $n $T) (result $T) (>>> (& $x $mask) $n))
      (func $main (export "main") (result i64)
        (if (result i64) (call (%inst $both i32) 2 4)
          (then (call (%inst $bits i64) 84 127 1)) (else 0))))"#,
        "42",
    );
    for (class, ty) in [
        ("Logical", "i64"),
        ("Logical", "f32"),
        ("Bitwise", "f64"),
        ("Shift", "v128"),
        ("Shift", "%unit"),
    ] {
        error(
            &format!(
                "(module (func $require (%type.params $T) (%where ({class} $T)) (nop)) (func $unused (call (%inst $require {ty}))))"
            ),
            "E_GENERIC_BOUND",
        );
    }
    for clause in ["(Logical)", "(Bitwise i32)", "(Shift $Missing)"] {
        error(
            &format!("(module (func $f (%type.params $T) (%where {clause}) (nop)))"),
            "E_GENERIC_CONSTRAINT",
        );
    }
}

#[test]
fn numeric_bit_operations_require_matching_integer_widths_and_known_types() {
    for expression in [
        "(<< (i64.const 1) (i32.const 1))",
        "(& (i64.const 1) (i32.const 1))",
    ] {
        error(
            &format!("(module (func $f (result i64) {expression}))"),
            "E_TYPE_MISMATCH",
        );
    }
    for (ty, expression) in [("f64", "(<< 1 2)"), ("v128", "(& $x $x)")] {
        let params = if ty == "v128" { "(param $x v128)" } else { "" };
        error(
            &format!("(module (func $f {params} (result {ty}) {expression}))"),
            "E_CLASS_INSTANCE",
        );
    }
    error(
        "(module (func $f (result i32) (%let $x (<< 1 2) $x)))",
        "E_OPERATOR_TYPE",
    );
    for expression in ["(&& 1)", "(|| 0 1 2)", "(<<)", "(>> 1)", "(>>> 1 2 3)"] {
        error(
            &format!("(module (func $f (result i32) {expression}))"),
            "E_OPERATOR_ARITY",
        );
    }
}

#[test]
fn conditional_rhs_cannot_consume_an_outer_owner_on_only_one_path() {
    for op in ["&&", "||"] {
        let source = format!(
            "(module (func $f (param $flag i32)
          (%let $x (%box.new i32 42)
            (block (drop ({op} $flag (block (result i32) (%box.close (%move $x)) 1)))
              (%box.close (%move $x))))))"
        );
        let diagnostic = wlt::check(&source).unwrap_err();
        assert!(diagnostic.to_string().contains("ownership"), "{diagnostic}");
    }
    let source = r#"(module (func $main (export "main") (result i32) (%effect pure)
      (&& 1 (%let $x (%box.new i32 42) (block (result i32) (%box.close (%move $x)) 9)))))"#;
    wlt::verified::compile(source).unwrap();
    run(source, "1");
}

#[test]
fn inspection_and_formatting_keep_logical_and_shift_operand_types() {
    let source = "(module (func $f (param $x i64) (result i64) (>>> $x 1)))";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    std::fs::write(&path, source).unwrap();
    let loaded = wlt::source::load(path).unwrap();
    for needle in [">>>", "$x 1", "1)))"] {
        let query = wlt::inspect::type_at(
            &loaded,
            None,
            1,
            source.find(needle).unwrap() + 1,
            &wlt::bindings::BindingPolicy::default(),
        )
        .unwrap();
        assert_eq!(query.mode, "typed");
        assert_eq!(query.matches[0].ty, "i64");
    }
    let short = "(module (func $main (export \"main\") (result i32) (%let $x (type i64) (<< 1 42) (&& (i32.wrap_i64 (>>> $x 40)) 2))))";
    let formatted = wlt::format::source(short).unwrap();
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(short).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn logical_and_shift_laws_reuse_the_existing_proof_semantics() {
    let source = r#"(module
      (%law "canonical truth" (param $x i32) (= (|| $x 0) (i32.ne $x 0)))
      (%law "masked shift" (param $x i64) (= (<< $x 64) $x))
      (func $main (export "main") (result i32) (%effect pure) (| 40 2)))"#;
    wlt::verified::compile(source).unwrap();
}
