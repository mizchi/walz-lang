use gcwat::compile;
use std::process::{Command, Output};

fn invoke(wasm: &[u8]) -> Output {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("variants.wasm");
    std::fs::write(&path, wasm).unwrap();
    Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            "main",
        ])
        .arg(path)
        .output()
        .unwrap()
}

fn run(source: &str) -> String {
    let output = invoke(&compile(source).unwrap().wasm);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn foreign_base_values_and_inconsistent_tags_trap() {
    let compiled = compile(
        r#"(module
      (type $T (variant (empty) (value i64)))
      (func $read (param $x (ref $T)) (result i64)
        (match $T (result i64) (local.get $x)
          (case empty () (i64.const 0))
          (case value ($n) (local.get $n)))))"#,
    )
    .unwrap();
    // Insert foreign standard WAT that the source checker deliberately rejects.
    // Cover a valid tag on the base type, an unknown tag, and a wrong payload shape.
    for foreign in [
        "(struct.new $T (i32.const 0))",
        "(struct.new $T (i32.const 99))",
        "(struct.new $__variant_0_0 (i32.const 1))",
    ] {
        let mut wat = compiled.wat.clone();
        wat.insert_str(
            wat.rfind(')').unwrap(),
            &format!("(func (export \"main\") (result i64) (call $read {foreign}))"),
        );
        let wasm = wat::parse_str(wat).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let output = invoke(&wasm);
        assert!(
            !output.status.success(),
            "foreign value was accepted: {foreign}"
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("cast failure") || error.contains("unreachable"),
            "{error}"
        );
    }
}

#[test]
fn cases_with_identical_storage_remain_distinct_and_order_independent() {
    assert_eq!(
        run(r#"(module
      (type $Choice (variant (first i64) (second i64) (off) (on)))
      (func $read (param $x (ref $Choice)) (result i64)
        (match $Choice (result i64) (local.get $x)
          (case on () (i64.const 100))
          (case second ($n) (i64.add (local.get $n) (i64.const 2)))
          (case first ($n) (local.get $n))
          (case off () (i64.const 0))))
      (func $main (export "main") (result i64)
        (i64.add
          (i64.add (call $read (variant.new $Choice first (i64.const 10)))
                   (call $read (variant.new $Choice second (i64.const 20))))
          (i64.sub (call $read (variant.new $Choice on))
                   (i64.add (call $read (variant.new $Choice off)) (i64.const 90))))))"#),
        "42"
    );
}

#[test]
fn recursive_variants_support_tail_recur_and_reference_payloads() {
    assert_eq!(
        run(r#"(module
      (type $List (variant (nil) (cons i64 (ref $List))))
      (func $sum (param $xs (ref $List)) (result i64)
        (loop $walk (bind $rest (ref $List) (local.get $xs))
          (bind $acc i64 (i64.const 0)) (result i64)
          (match $List (result i64) (local.get $rest)
            (case nil () (local.get $acc))
            (case cons ($head $tail)
              (recur $walk (local.get $tail) (i64.add (local.get $acc) (local.get $head)))))))
      (func $main (export "main") (result i64)
        (call $sum (variant.new $List cons (i64.const 20)
          (variant.new $List cons (i64.const 22) (variant.new $List nil))))))"#),
        "42"
    );
}

#[test]
fn subject_runs_once_and_only_selected_arm_runs() {
    assert_eq!(
        run(r#"(module
      (type $Cell (struct (field $value (mut i64))))
      (type $Result (variant (ok i64) (error i32)))
      (func $next (param $cell (ref $Cell)) (result (ref $Result))
        (block (result (ref $Result))
          (struct.set $Cell $value (local.get $cell)
            (i64.add (struct.get $Cell $value (local.get $cell)) (i64.const 1)))
          (variant.new $Result ok (i64.const 41))))
      (func $main (export "main") (result i64)
        (let $cell (struct.new $Cell (i64.const 0))
          (i64.add
            (match $Result (result i64) (call $next (local.get $cell))
              (case error ($code) (unreachable))
              (case ok ($value) (local.get $value)))
            (struct.get $Cell $value (local.get $cell))))))"#),
        "42"
    );
}

#[test]
fn constructor_payloads_run_once_from_left_to_right() {
    assert_eq!(
        run(r#"(module
      (type $Cell (struct (field $value (mut i64))))
      (type $Pair (variant (pair i64 i64)))
      (func $next (param $cell (ref $Cell)) (result i64)
        (block (result i64)
          (struct.set $Cell $value (local.get $cell)
            (i64.add (struct.get $Cell $value (local.get $cell)) (i64.const 1)))
          (struct.get $Cell $value (local.get $cell))))
      (func $main (export "main") (result i64)
        (let $cell (struct.new $Cell (i64.const 0))
          (match $Pair (result i64)
            (variant.new $Pair pair (call $next (local.get $cell)) (call $next (local.get $cell)))
            (case pair ($a $b)
              (i64.add (i64.const 30) (i64.add (i64.mul (local.get $a) (i64.const 10)) (local.get $b))))))))"#),
        "42"
    );
}

#[test]
fn payload_bindings_are_lexical_and_can_be_captured_explicitly() {
    assert_eq!(
        run(r#"(module
      (type $Option (variant (none) (some i64)))
      (type $Options (array (ref $Option)))
      (closure $Value (capture $n i64) (result i64) (local.get $n))
      (func $main (export "main") (result i64)
        (let $n (i64.const 2)
          (let $xs (array.new_fixed $Options 1 (variant.new $Option some (i64.const 40)))
            (let $f
              (match $Option (result (ref $Value)) (array.get $Options (local.get $xs) (i32.const 0))
                (case none () (closure.new $Value (i64.const 0)))
                (case some ($n) (closure.new $Value (local.get $n))))
              (i64.add (local.get $n) (closure.call $Value (local.get $f))))))))"#),
        "42"
    );
}

#[test]
fn nested_and_unit_matches_preserve_control_flow() {
    assert_eq!(
        run(r#"(module
      (type $Flag (variant (off) (on)))
      (func $main (export "main") (result i64)
        (block (result i64)
          (match $Flag (variant.new $Flag off)
            (case on () (unreachable)) (case off () (nop)))
          (match $Flag (result i64) (variant.new $Flag on)
            (case off () (unreachable))
            (case on ()
              (match $Flag (result i64) (variant.new $Flag off)
                (case on () (unreachable)) (case off () (i64.const 42))))))))"#),
        "42"
    );
}

#[test]
fn invalid_declarations_and_constructors_are_rejected() {
    for (source, diagnostic) in [
        ("(module (type $T (variant)))", "at least one variant"),
        (
            "(module (type $T (variant (a) (a i64))))",
            "duplicate variant",
        ),
        ("(module (type $T (variant ($a))))", "variant identifier"),
        (
            "(module (type $T (variant (a (mut i64)))))",
            "expected numeric type",
        ),
        (
            "(module (type $T (variant (a))) (func $f (result (ref $T)) (variant.new $T b)))",
            "unknown variant",
        ),
        (
            "(module (type $T (variant (literal i64) (add (ref $T) (ref $T)))) (func $f (result (ref $T)) (variant.new $T add (ref.null $T) (ref.null $T))))",
            "type mismatch",
        ),
        (
            "(module (type $T (variant (a i64))) (func $f (result (ref $T)) (variant.new $T a)))",
            "argument count",
        ),
        (
            "(module (type $T (variant (a i64))) (func $f (result (ref $T)) (variant.new $T a (i32.const 1))))",
            "type mismatch",
        ),
        (
            "(module (type $T (variant (a))) (func $f (result (ref $T)) (struct.new $T)))",
            "constructor does not match type declaration",
        ),
        (
            "(module (type $T (variant (a))) (func $f (param $x (ref $T)) (result i32) (struct.get $T 0 (local.get $x))))",
            "expected struct type",
        ),
        (
            "(module (type $T (variant (a))) (func $f (param $x (ref $T)) (struct.set $T 0 (local.get $x) (i32.const 0))))",
            "expected struct type",
        ),
        (
            "(module (type $S (struct)) (func $f (result (ref $S)) (variant.new $S a)))",
            "expected variant type",
        ),
        (
            "(module (type $A (variant (a))) (type $B (variant (a))) (func $f (result (ref $A)) (variant.new $B a)))",
            "type mismatch",
        ),
    ] {
        let error = compile(source).err().unwrap().to_string();
        assert!(
            error.contains(diagnostic),
            "{source}\n{error}; expected {diagnostic}"
        );
    }
}

#[test]
fn matches_require_exhaustive_unique_well_typed_cases() {
    let prefix =
        "(module (type $T (variant (a i64) (b))) (func $f (param $x (ref $T)) (result i64) ";
    for (body, diagnostic) in [
        (
            "(match $T (result i64) (local.get $x) (case a ($n) (local.get $n)))",
            "non-exhaustive match: missing b",
        ),
        (
            "(match $T (result i64) (local.get $x) (case a ($n) (local.get $n)) (case a ($m) (local.get $m)) (case b () (i64.const 0)))",
            "duplicate match case",
        ),
        (
            "(match $T (result i64) (local.get $x) (case c () (i64.const 0)))",
            "unknown variant",
        ),
        (
            "(match $T (result i64) (local.get $x) (case a () (i64.const 0)) (case b () (i64.const 0)))",
            "payload binding count",
        ),
        (
            "(match $T (result i64) (local.get $x) (case a ($n) (i32.const 0)) (case b () (i64.const 0)))",
            "type mismatch",
        ),
        (
            "(match $T (result i64) (local.get $x) (case a ($n) (local.get $n)) (case b () (local.get $n)))",
            "unbound local",
        ),
        (
            "(match $T (result i64) (ref.null $T) (case a ($n) (local.get $n)) (case b () (i64.const 0)))",
            "type mismatch",
        ),
        (
            "(match $T (result i64) (i64.const 0) (case a ($n) (local.get $n)) (case b () (i64.const 0)))",
            "type mismatch",
        ),
    ] {
        let source = format!("{prefix}{body}))");
        let error = compile(&source).err().unwrap().to_string();
        assert!(
            error.contains(diagnostic),
            "{source}\n{error}; expected {diagnostic}"
        );
    }
    let error = compile("(module (type $T (variant (a i64 i64))) (func $f (param $x (ref $T)) (match $T (local.get $x) (case a ($n $n) (nop)))))").err().unwrap().to_string();
    assert!(error.contains("duplicate payload binding"), "{error}");
}

#[test]
fn analysis_visits_subject_and_all_arms() {
    let compiled = compile(
        r#"(module
      (type $T (variant (a i64) (b)))
      (func $left (param $n i64) (result i64) (local.get $n))
      (func $right (result i64) (i64.const 0))
      (func $subject (param $x (ref $T)) (result (ref $T)) (local.get $x))
      (func $choose (param $x (ref $T)) (result i64)
        (match $T (result i64) (call $subject (local.get $x))
          (case a ($n) (call $left (local.get $n)))
          (case b () (call $right))))
      (func $make (result (ref $T)) (variant.new $T a (i64.const 42))))"#,
    )
    .unwrap();
    let choose = compiled
        .analysis
        .functions
        .iter()
        .find(|f| f.name == "$choose")
        .unwrap();
    assert_eq!(choose.calls, ["$left", "$right", "$subject"]);
    assert_eq!(
        choose.effects,
        gcwat::Effects {
            may_trap: true,
            ..Default::default()
        }
    );
    let make = compiled
        .analysis
        .functions
        .iter()
        .find(|f| f.name == "$make")
        .unwrap();
    assert!(make.effects.allocates && make.effects.may_trap);
}

#[test]
fn malformed_variant_syntax_has_source_diagnostics() {
    for source in [
        "(module (type $T (variant ())))",
        "(module (type $T (variant a)))",
        "(module (type $T (variant (a))) (func $f (variant.new)))",
        "(module (type $T (variant (a))) (func $f (match)))",
        "(module (type $T (variant (a))) (func $f (match $T)))",
        "(module (type $T (variant (a))) (func $f (match $T (variant.new $T a) (case a))))",
        "(module (type $T (variant (a))) (func $f (match $T (variant.new $T a) (other a () (nop)))))",
        "(module (type $T (variant (a))) (func $f (match $T (variant.new $T a) (case a x (nop)))))",
    ] {
        let error = compile(source).err().unwrap().to_string();
        assert!(error.starts_with("1:"), "{error}");
        assert!(!error.contains("internal"), "{error}");
    }
}
