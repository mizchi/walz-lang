use std::process::Command;

const GOOD: &str = r#"(module
  (tag $E)
  (func $fail (param $flag i32) (%effect (throws $E))
    (if (local.get $flag) (then (throw $E)) (else (nop))))
  (func $run (param $flag i32) (result i32) (%effect pure)
    (%let $x (%box.new i32 (i32.const 42))
      (%try (result i32)
        (do (block (result i32) (call $fail (local.get $flag))
          (%match (%box i32) (result i32) (%move $x) (case own ($n) (local.get $n)))))
        (catch $E () (%match (%box i32) (result i32) (%move $x) (case own ($n) (local.get $n)))))))
  (func $main (export "main") (result i32) (%effect pure)
    (i32.add (call $run (i32.const 0)) (call $run (i32.const 1)))))"#;

#[test]
fn owners_survive_caught_exceptions_and_both_paths_are_independently_verified() {
    let compiled = wlt::verified::compile(GOOD).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exceptions.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    let out = Command::new("wasmtime")
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
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "84");
}

#[test]
fn exceptional_edges_cannot_lose_owners_or_uncommitted_owned_arguments() {
    assert!(wlt::check(&GOOD.replace("(catch $E () (%match (%box i32) (result i32) (%move $x) (case own ($n) (local.get $n))))", "(catch $E () (i32.const 42))")).is_err());
    let pending = r#"(module
      (tag $E)
      (func $fail (result i32) (%effect (throws $E)) (throw $E))
      (func $take (param $x (%box i32)) (param $n i32) (%effect pure) (%box.close (%move $x)))
      (func $f (%effect pure)
        (%try (do (call $take (%box.new i32 (i32.const 1)) (call $fail))) (catch $E () (nop)))))"#;
    let error = wlt::check(pending).unwrap_err().to_string();
    assert!(error.contains("temporary"), "{error}");
}

#[test]
fn nested_handlers_and_throwing_closures_preserve_owner_states() {
    let nested = r#"(module (tag $A) (tag $B)
      (%closure $raise (%effect (throws $A)) (throw $A))
      (func $f (param $x (%box i32)) (%effect allocates)
        (%try
          (do (block
            (%try (do (%closure.call $raise (%closure.new $raise)))
              (catch $A () (throw $B)))
            (%box.close (%move $x))))
          (catch $B () (%box.close (%move $x))))))"#;
    wlt::verified::compile(nested).unwrap();
    let moved = GOOD.replace(
        "(call $fail (local.get $flag))",
        "(block (%box.close (%move $x)) (call $fail (local.get $flag)))",
    );
    assert!(wlt::check(&moved).is_err());
    let unknown = r#"(module
      (import "host" "maybe" (func $maybe))
      (func $f (param $x (%box i32))
        (%try (do (call $maybe)) (catch_all (nop)))
      ))"#;
    assert!(wlt::check(unknown).is_err());
}

#[test]
fn explicit_throw_cannot_abandon_a_previously_evaluated_owned_operand() {
    let source = r#"(module (tag $E)
      (func $consume (param $x (%box i32)) (param $n i32) (%box.close (%move $x)))
      (func $f (call $consume (%box.new i32 (i32.const 1)) (throw $E))))"#;
    let error = wlt::check(source).unwrap_err().to_string();
    assert!(error.contains("temporary"), "{error}");
}
