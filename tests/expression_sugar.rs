// Compiler scope fixtures use private expanded forms; public files use %use paths.
use std::process::Command;

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compiled = wlt::compile_with_options(source, wlt::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.wasm");
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

#[test]
fn short_locals_and_constants_emit_the_same_wasm_as_explicit_expressions() {
    let short = r#"(module
      (func $main (export "main") (result i64) (%effect pure)
        (%let (($a (type i64) 40) ($b $a)) (i64.add $b 2))))"#;
    let explicit = short
        .replace("(type i64) 40", "(type i64) (i64.const 40)")
        .replace("($b $a)", "($b (local.get $a))")
        .replace("(i64.add $b 2)", "(i64.add (local.get $b) (i64.const 2))");
    for optimize in [false, true] {
        let options = wlt::CompileOptions { optimize };
        let short = wlt::compile_with_options(short, options).unwrap();
        assert_eq!(
            short.wasm,
            wlt::compile_with_options(&explicit, options).unwrap().wasm
        );
        assert!(!short.analysis.opaque);
    }
    run(short, "42");
}

#[test]
fn operators_determine_operand_types_independently_of_the_result_type() {
    run(
        r#"(module (func $main (export "main") (result i32)
      (if (result i32)
        (i32.and (i64.eq 0xffff_ffff_ffff_ffff -1)
          (i32.and (f32.eq 0x1.8p+1 3) (f64.eq -inf -inf)))
        (then (i32.wrap_i64 (i64.add 40 2)))
        (else 0))))"#,
        "42",
    );
}

#[test]
fn declarations_supply_types_for_calls_results_conditions_arrays_and_fields() {
    run(
        r#"(module
      (type $A (array (mut i64)))
      (type $S (struct (field $n i64)))
      (func $id (param $x i64) (result i64) $x)
      (func $answer (result i64) 42)
      (func $main (export "main") (result i64)
        (%let (($a (array.new $A 0 2)) ($s (%$S::{:n (call $answer)})))
          (block (result i64)
            (array.set $A $a 1 (call $id (struct.get $S $n $s)))
            (if (result i64) 1
              (then (return (array.get $A $a 1)))
              (else 0))))))"#,
        "42",
    );
}

#[test]
fn short_local_resolution_keeps_lexical_scope_and_field_labels_separate() {
    run(
        r#"(module
      (type $S (struct (field $n i32)))
      (func $main (export "main") (result i32)
        (%let (($n (type i32) 40) ($saved $n) ($n (type i32) 2)
               ($s (%$S::{:n (i32.add $saved $n)})))
          (struct.get $S $n $s))))"#,
        "42",
    );
    for (source, message) in [
        ("(module (func $f (result i32) $missing))", "unbound local"),
        (
            "(module (global $x i32 (i32.const 1)) (func $f (result i32) $x))",
            "unsupported declaration",
        ),
        (
            "(module (type $S (struct (field $n i32))) (func $f (result (ref $S)) (%$S::{:n $n})))",
            "unbound local",
        ),
        (
            "(module (%closure $C (result i32) $x) (func $f (param $x i32) (drop (%closure.new $C))))",
            "unbound local",
        ),
    ] {
        let error = wlt::compile(source).err().expect("invalid source accepted");
        assert!(error.to_string().contains(message), "{source}: {error}");
        assert!(error.diagnostic().primary.is_some());
    }
}

#[test]
fn generic_functions_closures_match_and_loop_bindings_use_the_same_rules() {
    run(
        r#"(module
      (func $id (%type.params $T) (%where (shared $T)) (param $x $T) (result $T) $x)
      (type $E (%variant (value i64)))
      (%closure $Add (%capture $base i64) (param $n i64) (result i64) (i64.add $base $n))
      (func $main (export "main") (result i64)
        (%let (($c (%closure.new $Add 40))
               ($e (%variant.new $E value (call (%inst $id i64) 2))))
          (%match $E (result i64) $e
            (case value ($n)
              (%loop $again (bind $left i64 $n) (result i64)
                (if (result i64) (i64.eqz $left)
                  (then (%closure.call $Add $c $n))
                  (else (%recur $again (i64.sub $left 1))))))))))"#,
        "42",
    );
}

#[test]
fn numeric_elision_requires_a_known_scalar_type_and_valid_wat_literal() {
    for (source, message) in [
        (
            "(module (func $f (result i32) (%let $x 1 $x)))",
            "expected numeric type",
        ),
        ("(module (func $f (drop 1)))", "expected numeric type"),
        (
            "(module (type $Id (%newtype i32)) (func $f (result $Id) 1))",
            "expected numeric type",
        ),
        (
            "(module (type $S (struct)) (func $f (result (ref $S)) 1))",
            "expected numeric type",
        ),
        (
            "(module (func $f (result v128) 1))",
            "expected numeric type",
        ),
        (
            "(module (func $f (result i32) 4294967296))",
            "invalid numeric literal",
        ),
        (
            "(module (func $f (result i32) 1.5))",
            "invalid numeric literal",
        ),
        (
            "(module (func $f (result i64) (i32.const 1)))",
            "type mismatch",
        ),
        (
            "(module (func $f (param $x i32) (result i64) (i64.add $x 1)))",
            "type mismatch",
        ),
    ] {
        let error = wlt::compile(source).err().expect("invalid source accepted");
        assert!(error.to_string().contains(message), "{source}: {error}");
        assert!(error.diagnostic().primary.is_some());
    }
}

#[test]
fn shorthand_does_not_move_owned_values_or_skip_static_contracts() {
    for body in [
        "(%box.close $x)",
        "(%let $alias $x (%box.close (%move $alias)))",
    ] {
        let source = format!("(module (func $f (%let $x (%box.new i64 1) {body})))");
        let error = wlt::compile(&source)
            .err()
            .expect("invalid source accepted");
        assert!(error.to_string().contains("explicit move"), "{error}");
    }
    run(
        r#"(module (func $main (export "main") (result i64)
      (%let $x (%box.new i64 42)
        (%match (%box i64) (result i64) (%move $x) (case own ($n) $n)))))"#,
        "42",
    );
    let error = wlt::compile(
        r#"(module
      (import "host" "f" (func $host (param $n i32) (result i32)))
      (func $f (result i32) (%effect pure) (call $host 1)))"#,
    )
    .err()
    .expect("invalid source accepted");
    assert!(error.to_string().contains("effect contract"), "{error}");
}

#[test]
fn short_expressions_survive_formatting_and_native_wat_remains_unchanged() {
    let source = r#"(module (func $main (export "main") (result i32)
      (%let $foo (type i32) 41 (i32.add $foo 1))))"#;
    let formatted = wlt::format::source(source).unwrap();
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
    let native = r#"(module (memory 1) (data (i32.const 0) "$foo 1")
      (func $f (param $foo i32) (result i32) local.get $foo i32.const 1 i32.add))"#;
    assert_eq!(wlt::compile(native).unwrap().wat, native);
}

#[test]
fn let_type_annotations_keep_aliases_generics_references_and_native_wat() {
    run(
        r#"(module
      (%type.alias $Count i64)
      (type $Box (%type.params $T) (%where (shared $T) (nonunit $T)) (struct (field $n $T)))
      (func $main (export "main") (result i64)
        (%let (($x (type $Count) 42)
               ($b (type (ref (%inst $Box $Count))) (%struct.new (%inst $Box $Count) {:n $x})))
          (struct.get (%inst $Box $Count) $n $b))))"#,
        "42",
    );
    let native = r#"(module (memory 1)
      (func $main (export "main") (result i32)
        (%let $n (type i32) (i32.const 42)
          (block (result i32) (i32.store (i32.const 0) (local.get $n)) (i32.load (i32.const 0))))))"#;
    let compilation = wlt::compile(native).unwrap();
    assert!(compilation.analysis.opaque);
    for annotation in ["(type)", "(type i32 i64)"] {
        let source = format!("(module (func $f (result i32) (%let $x {annotation} 1 $x)))");
        let error = wlt::compile(&source)
            .err()
            .expect("invalid annotation accepted");
        assert_eq!(error.diagnostic().code, "E_LET_TYPE", "{error}");
    }
}

#[test]
fn inferred_value_forms_do_not_confuse_local_atoms_with_types() {
    run(
        r#"(module
      (%use "wlt:core" (type $tuple) (type $option) (type $result))
      (%type.alias $N i64)
      (func $f (result (%inst $result i64 i32))
        (%let (($N (type i64) 40)
               ($p (type (%inst $tuple i64 i64)) (%tuple.new $N 2))
               ($o (type (%inst $option i64)) (%option.some $N))
               ($r (type (%inst $result i64 i32)) (%result.ok 2)))
          (%match $p
            (case pair ($a $b)
              (%result.let ($n) $r
                (%match $o
                  (case none () (%result.err i64 i32 1))
                  (case some ($x) (%result.ok i64 i32 (i64.add $x $n)))))))))
      (func $main (export "main") (result i64)
        (%match (call $f) (case ok ($n) $n) (case err ($e) -1))))"#,
        "42",
    );
}

#[test]
fn result_do_bound_locals_are_values_even_when_they_share_a_type_name() {
    run(
        r#"(module
      (%use "wlt:core" (type $option) (type $result))
      (%type.alias $N i64)
      (func $f (result (%inst $result (%inst $option i64) i32))
        (%result.do ($N (%result.ok i64 i32 40))
          (%result.ok (%option.some $N))))
      (func $main (export "main") (result i64)
        (%match (call $f)
          (case err ($e) -1)
          (case ok ($o) (%match $o
            (case none () -1)
            (case some ($n) (i64.add $n 2)))))))"#,
        "42",
    );
}

#[test]
fn inspection_keeps_the_source_type_and_spans_of_short_expressions() {
    let source = "(module (%type.alias $Count i64) (func $f (result i64) (%let $n (type $Count) 41 (i64.add $n 1))))";
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.wlt");
    std::fs::write(&path, source).unwrap();
    let loaded = wlt::source::load(path).unwrap();
    for (needle, source_type) in [
        ("$n (type", Some("$Count")),
        ("41", None),
        ("$n 1", Some("$Count")),
        ("1))))", None),
    ] {
        let column = source.find(needle).unwrap() + 1;
        let query = wlt::inspect::type_at(
            &loaded,
            None,
            1,
            column,
            &wlt::bindings::BindingPolicy::default(),
        )
        .unwrap();
        assert_eq!(query.mode, "typed");
        let found = &query.matches[0];
        assert_eq!(found.ty, "i64", "{query:?}");
        assert_eq!(found.source_type.as_deref(), source_type, "{query:?}");
        assert!(
            found.location.span.start < column && column - 1 < found.location.span.end,
            "{query:?}"
        );
    }
}

#[test]
fn namespace_type_resolution_preserves_short_local_reads_and_explicit_type_operands() {
    run(
        r#"(module
      (%use "wlt:core" (type $option))
      (%legacy.namespace $n
        (%use "wlt:core" (type $option))
        (%public (func $f))
        (%type.alias $N i64)
        (func $f (param $N i64) (result (%inst $option i64))
          (%let $v (type (%inst $option i64)) (%option.some $N $N)
            (%match $v (case none () (%option.none i64))
              (case some ($N) (%option.some $N))))))
      (func $main (export "main") (result i64)
        (%match (call $n.f 40) (case none () -1) (case some ($n) (i64.add $n 2)))))"#,
        "42",
    );
}

#[test]
fn simd_operands_and_explicit_newtype_wraps_provide_scalar_expectations() {
    run(
        r#"(module
      (type $N (%newtype i64))
      (func $main (export "main") (result i32)
        (%let (($n (type $N) (%newtype.wrap $N 40))
               ($v (type v128) (i32x4.splat (i32.wrap_i64 (%newtype.unwrap $N $n)))))
          (i32.add (i32x4.extract_lane 0 $v) 2))))"#,
        "42",
    );
}

#[test]
fn malformed_declarations_are_diagnostics_and_not_panics() {
    for source in ["(module (func))", "(module (%closure))", "(module (%law))"] {
        assert!(wlt::check(source).is_err(), "{source}");
    }
    // An anonymous empty function is valid native WAT, even though the typed
    // core requires a name. Its optional analysis must not panic either.
    assert!(wlt::compile("(module (func))").is_ok());
}

#[test]
fn generic_constructor_type_operands_survive_parameter_renaming() {
    run(
        r#"(module
      (%use "wlt:core" (type $option))
      (func $empty (%type.params $T) (%where (shared $T) (nonunit $T))
        (result (%inst $option $T)) (%option.none $T))
      (func $wrap (%type.params $T) (%where (shared $T) (nonunit $T))
        (param $x $T) (result (%inst $option $T)) (%option.some $T $x))
      (func $main (export "main") (result i64)
        (%match (call (%inst $empty i64))
          (case none () (%match (call (%inst $wrap i64) 42)
            (case none () 0) (case some ($n) $n)))
          (case some ($n) -1))))"#,
        "42",
    );
}

#[test]
fn literal_sugar_does_not_claim_native_stack_instruction_tokens() {
    for body in ["nop", "unreachable"] {
        let source = format!("(module (func $f (%let $x (type i32) (i32.const 1) {body})))");
        let compiled = wlt::compile(&source).unwrap();
        assert!(compiled.analysis.opaque);
    }
}
