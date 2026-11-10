use std::process::Command;
use walz::compile;

fn run(source: &str) -> String {
    let compiled = compile(source).unwrap();
    wasmparser::Validator::new()
        .validate_all(&compiled.wasm)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("program.wasm");
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
        .expect("runtime tests require wasmtime on PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn folded_wat_and_immutable_lexical_bindings() {
    assert_eq!(
        run(r#"(module
      (func $double (param $x i64) (result i64)
        (i64.mul (local.get $x) (i64.const 2)))
      (func $main (export "main") (result i64)
        (@let $x (i64.const 20)
          (i64.add (call $double (local.get $x)) (i64.const 2)))))"#),
        "42"
    );
    assert_eq!(
        run(r#"(module (func $main (export "main") (result i64)
      (@let $x (i64.const 10)
        (i64.add (@let $x (i64.const 20) (local.get $x)) (local.get $x)))))"#),
        "30"
    );
}

#[test]
fn gc_structs_recursive_types_and_explicit_mutation() {
    assert_eq!(
        run(r#"(module
      (type $Cell (struct (field $value (mut i64))))
      (type $Node (struct (field $value i64) (field $next (ref null $Node))))
      (func $main (export "main") (result i64)
        (@let $cell (struct.new $Cell (i64.const 0))
          (@let $node (struct.new $Node (i64.const 42) (ref.null $Node))
            (block (result i64)
              (struct.set $Cell $value (local.get $cell)
                (struct.get $Node $value (local.get $node)))
              (struct.get $Cell $value (local.get $cell)))))))"#),
        "42"
    );
}

#[test]
fn arrays_and_loops_with_explicit_state() {
    assert_eq!(
        run(r#"(module
      (type $Ints (array (mut i64)))
      (func $main (export "main") (result i64)
        (@let $xs (array.new_fixed $Ints 3 (i64.const 10) (i64.const 20) (i64.const 12))
          (@loop $sum
            (bind $i i32 (i32.const 0))
            (bind $acc i64 (i64.const 0))
            (result i64)
            (if (result i64) (i32.lt_s (local.get $i) (array.len (local.get $xs)))
              (then (@recur $sum
                (i32.add (local.get $i) (i32.const 1))
                (i64.add (local.get $acc) (array.get $Ints (local.get $xs) (local.get $i)))))
              (else (local.get $acc)))))))"#),
        "42"
    );
}

#[test]
fn recur_updates_all_state_simultaneously() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i64)
        (@loop $swap
          (bind $a i64 (i64.const 1))
          (bind $b i64 (i64.const 2))
          (bind $i i32 (i32.const 0))
          (result i64)
          (if (result i64) (i32.lt_s (local.get $i) (i32.const 3))
            (then (@recur $swap (local.get $b) (local.get $a)
              (i32.add (local.get $i) (i32.const 1))))
            (else (i64.add (i64.mul (local.get $a) (i64.const 10)) (local.get $b)))))))"#),
        "21"
    );
}

#[test]
fn closures_have_explicit_captures_and_can_escape_their_creator() {
    let source = r#"(module
      (@closure $Adder (@capture $base i64) (param $x i64) (result i64)
        (i64.add (local.get $base) (local.get $x)))
      (func $make (param $n i64) (result (ref $Adder))
        (@closure.new $Adder (local.get $n)))
      (func $main (export "main") (result i64)
        (@let $f (call $make (i64.const 40))
          (@closure.call $Adder (local.get $f) (i64.const 2)))))"#;
    assert_eq!(run(source), "42");
    let compiled = compile(source).unwrap();
    let summary = compiled
        .analysis
        .functions
        .iter()
        .find(|f| f.name == "$Adder")
        .unwrap();
    assert_eq!(summary.captures, ["$base"]);
    assert_eq!(summary.effects, walz::Effects::default());
}

#[test]
fn captured_references_share_explicit_mutable_objects() {
    assert_eq!(
        run(r#"(module
      (type $Cell (struct (field $value (mut i64))))
      (@closure $Increment (@capture $cell (ref $Cell)) (result i64)
        (block (result i64)
          (struct.set $Cell $value (local.get $cell)
            (i64.add (struct.get $Cell $value (local.get $cell)) (i64.const 1)))
          (struct.get $Cell $value (local.get $cell))))
      (func $main (export "main") (result i64)
        (@let $cell (struct.new $Cell (i64.const 40))
          (@let $f (@closure.new $Increment (local.get $cell))
            (block (result i64)
              (drop (@closure.call $Increment (local.get $f)))
              (@closure.call $Increment (local.get $f)))))))"#),
        "42"
    );
}

#[test]
fn effects_propagate_through_calls_and_recursion() {
    let compiled = compile(
        r#"(module
      (import "host" "read" (func $read (result i64)))
      (func $pure (param $x i64) (result i64) (i64.add (local.get $x) (i64.const 1)))
      (func $recursive (param $n i64) (result i64)
        (if (result i64) (i64.eqz (local.get $n))
          (then (i64.const 0))
          (else (call $recursive (i64.sub (local.get $n) (i64.const 1))))))
      (func $caller (result i64) (call $recursive (i64.const 3)))
      (func $host_caller (result i64) (call $read)))"#,
    )
    .unwrap();
    let effects = |name| {
        compiled
            .analysis
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .effects
    };
    assert_eq!(effects("$pure"), walz::Effects::default());
    assert!(effects("$recursive").may_diverge);
    assert!(effects("$caller").may_diverge);
    assert!(effects("$host_caller").host);
    assert!(effects("$host_caller").writes);
    assert!(effects("$host_caller").may_trap);
}

#[test]
fn rejects_programs_outside_the_analyzable_core() {
    for (source, expected) in [
        (
            "(module (func $f (result i64) (i32.const 1)))",
            "type mismatch",
        ),
        (
            "(module (func $f (result i64) (local.get $missing)))",
            "unbound local",
        ),
        (
            "(module (func $f (result i64) (local.set $x (i64.const 1))))",
            "unsupported expression",
        ),
        (
            "(module (func $f (result i64) (call $missing)))",
            "unknown function",
        ),
        (
            "(module (func $f (param $x i64) (result i64) (local.get $x)) (func $g (result i64) (call $f)))",
            "argument count",
        ),
        (
            "(module (type $T (struct (field $x i64))) (func $f (param $p (ref $T)) (struct.set $T $x (local.get $p) (i64.const 1))))",
            "immutable field",
        ),
        (
            "(module (type $T (array i64)) (func $f (param $p (ref $T)) (array.set $T (local.get $p) (i32.const 0) (i64.const 1))))",
            "immutable array",
        ),
        (
            "(module (@closure $C (param $x i64) (result i64) (i64.add (local.get $x) (local.get $hidden))))",
            "unbound local",
        ),
        (
            "(module (@closure $C (@capture $x i64) (param $x i64) (result i64) (local.get $x)))",
            "duplicate binding",
        ),
        (
            "(module (@closure $C (@capture $x i64) (result i64) (local.get $x)) (func $f (result (ref $C)) (@closure.new $C)))",
            "capture count",
        ),
        (
            "(module (func $f (result i64) (@recur $missing)))",
            "unknown loop",
        ),
        (
            "(module (func $f (result i64) (@loop $l (bind $x i64 (i64.const 0)) (result i64) (@recur $l (i32.const 1)))))",
            "type mismatch",
        ),
        (
            "(module (func $f (result i64) (@loop $l (bind $x i64 (i64.const 0)) (result i64) (i64.add (@recur $l (local.get $x)) (i64.const 1)))))",
            "tail position",
        ),
        ("(module (memory 1))", "unsupported declaration"),
        (
            "(module (func $f (param $x i64) (result i64) local.get $x))",
            "one expression as function body",
        ),
        ("(module (func $f) (func $f))", "duplicate function"),
        ("(module (type $T (array (ref $Missing))))", "unknown type"),
        ("(module (func $__reserved (nop)))", "reserved"),
    ] {
        let error = match walz::check(source) {
            Ok(_) => panic!("unexpectedly accepted: {source}"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains(expected),
            "{source}\n{error}\nexpected {expected}"
        );
    }
}

#[test]
fn effect_facts_distinguish_mutation_nullability_and_allocation() {
    let compiled = compile(
        r#"(module
      (type $Value (struct (field $x i64)))
      (type $Cell (struct (field $x (mut i64))))
      (type $Values (array i64))
      (func $value (param $p (ref $Value)) (result i64)
        (struct.get $Value $x (local.get $p)))
      (func $nullable (param $p (ref null $Value)) (result i64)
        (struct.get $Value $x (local.get $p)))
      (func $read (param $p (ref $Cell)) (result i64)
        (struct.get $Cell $x (local.get $p)))
      (func $write (param $p (ref $Cell))
        (struct.set $Cell $x (local.get $p) (i64.const 1)))
      (func $length (param $p (ref $Values)) (result i32)
        (array.len (local.get $p)))
      (func $element (param $p (ref $Values)) (result i64)
        (array.get $Values (local.get $p) (i32.const 0)))
      (func $allocate (result (ref $Value))
        (struct.new $Value (i64.const 1))))"#,
    )
    .unwrap();
    let effects = |name| {
        compiled
            .analysis
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .effects
    };
    assert_eq!(effects("$value"), walz::Effects::default());
    assert_eq!(effects("$length"), walz::Effects::default());
    assert!(effects("$nullable").may_trap);
    assert!(!effects("$nullable").reads);
    assert!(effects("$read").reads);
    assert!(!effects("$read").may_trap);
    assert!(effects("$write").writes);
    assert!(effects("$element").may_trap);
    assert!(!effects("$element").reads);
    assert!(effects("$allocate").allocates);
    assert!(effects("$allocate").may_trap);
}

#[test]
fn tail_recur_can_be_nested_under_let_and_unit_blocks() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i64)
        (@loop $go (bind $i i32 (i32.const 0)) (result i64)
          (if (result i64) (i32.lt_s (local.get $i) (i32.const 3))
            (then
              (@let $next (i32.add (local.get $i) (i32.const 1))
                (block (nop) (@recur $go (local.get $next)))))
            (else (i64.const 42))))))"#),
        "42"
    );
}

#[test]
fn constants_arrays_and_reference_operations_keep_wasm_semantics() {
    assert_eq!(
        run(r#"(module (func $main (export "main") (result i64)
      (i64.add (i64.const 9223372036854775807) (i64.const 1))))"#),
        i64::MIN.to_string()
    );
    assert_eq!(
        run(r#"(module (func $main (export "main") (result f64)
      (f64.add (f64.const 1.5) (f64.const 40.5))))"#),
        "42"
    );
    assert_eq!(
        run(r#"(module (type $Ints (array i64))
      (func $main (export "main") (result i64)
        (@let $xs (array.new $Ints (i64.const 7) (i32.const 6))
          (array.get $Ints (local.get $xs) (i32.const 5)))))"#),
        "7"
    );
    assert_eq!(
        run(r#"(module (type $Value (struct (field i64)))
      (func $main (export "main") (result i32)
        (ref.is_null (ref.null $Value))))"#),
        "1"
    );
    assert_eq!(
        run(r#"(module (type $Value (struct (field i64)))
      (func $main (export "main") (result i32)
        (@let $p (struct.new $Value (i64.const 1))
          (ref.eq (local.get $p) (local.get $p)))))"#),
        "1"
    );
}

#[test]
fn captures_are_value_snapshots_and_nominal_types_do_not_mix() {
    assert_eq!(
        run(r#"(module
      (@closure $C (@capture $x i64) (result i64) (local.get $x))
      (func $main (export "main") (result i64)
        (@let $x (i64.const 42)
          (@let $f (@closure.new $C (local.get $x))
            (@let $x (i64.const 100)
              (@closure.call $C (local.get $f)))))))"#),
        "42"
    );
    let source = r#"(module
      (@closure $A (result i64) (i64.const 1))
      (@closure $B (result i64) (i64.const 2))
      (func $f (result i64) (@closure.call $A (@closure.new $B))))"#;
    assert!(
        compile(source)
            .err()
            .unwrap()
            .to_string()
            .contains("type mismatch")
    );
}

#[test]
fn closure_environment_stays_live_during_allocation_pressure() {
    assert_eq!(
        run(r#"(module
      (type $Garbage (struct (field i32)))
      (@closure $C (@capture $x i64) (result i64) (local.get $x))
      (func $main (export "main") (result i64)
        (@let $f (@closure.new $C (i64.const 42))
          (@loop $go (bind $i i32 (i32.const 0)) (result i64)
            (if (result i64) (i32.lt_u (local.get $i) (i32.const 200000))
              (then (block (result i64)
                (drop (struct.new $Garbage (local.get $i)))
                (@recur $go (i32.add (local.get $i) (i32.const 1)))))
              (else (@closure.call $C (local.get $f))))))))"#),
        "42"
    );
}

#[test]
fn malformed_input_has_diagnostics_instead_of_panics() {
    for source in [
        "",
        "(",
        ")",
        "(module (type))",
        "(module (func))",
        "(module (type $T))",
        "(module (func $f (@let)))",
        "(module (func $f (array.new_fixed)))",
        "(module (; comment",
        "(module (import \"unterminated)",
    ] {
        assert!(walz::check(source).is_err(), "accepted {source:?}");
    }
    let deep = format!("{}0{}", "(".repeat(300), ")".repeat(300));
    assert!(
        compile(&deep)
            .err()
            .unwrap()
            .to_string()
            .contains("nesting limit")
    );
    assert_eq!(
        run(r#"(module
      (; Outer comment (; nested comment ;) 日本語 ;)
      (func $main (export "main") (result i64)
        ;; line comment
        (i64.const 42)))"#),
        "42"
    );
}
