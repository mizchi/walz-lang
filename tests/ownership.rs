use std::process::Command;

fn rejects(source: &str, expected: &str) {
    let error = wlt::check(source).unwrap_err().to_string();
    assert!(error.contains(expected), "{source}\n{error}");
}

fn run(source: &str) -> String {
    let compilation = wlt::compile(source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("owned.wasm");
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
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn owned_values_require_explicit_move_and_an_explicit_destination() {
    assert_eq!(
        run(r#"(module
      (func $take (param $x (%box i64)) (result i64) (%effect pure)
        (%match (%box i64) (result i64) (%move $x)
          (case own ($n) (local.get $n))))
      (func $main (export "main") (result i64) (%effect pure)
        (%let $x (%box.new i64 (i64.const 42)) (call $take (%move $x)))))"#),
        "42"
    );
    for (body, expected) in [
        ("(%box.close (local.get $x))", "explicit move"),
        (
            "(block (%box.close (%move $x)) (%box.close (%move $x)))",
            "already moved",
        ),
        ("(nop)", "unconsumed owned"),
        ("(drop (%move $x))", "cannot drop an owned"),
    ] {
        rejects(
            &format!("(module (func $f (%let $x (%box.new i64 (i64.const 1)) {body})))"),
            expected,
        );
    }
    rejects(
        "(module (func $f (param $n i64) (result i64) (%move $n)))",
        "move requires an owned",
    );
}

#[test]
fn ownership_propagates_through_pairs_options_results_and_once_closures() {
    assert_eq!(
        run(r#"(module
      (%closure $Take (%capture $x (%box i64)) (result i64) (%effect pure)
        (%match (%box i64) (result i64) (%move $x) (case own ($n) (local.get $n))))
      (func $main (export "main") (result i64) (%effect allocates)
        (%let $wrapped
          (%option.some (%result (%tuple (%box i64) i64) i32)
            (%result.ok (%tuple (%box i64) i64) i32
              (%tuple.new (%box i64) i64 (%box.new i64 (i64.const 40)) (i64.const 2))))
          (%match (%option (%result (%tuple (%box i64) i64) i32)) (result i64) (%move $wrapped)
            (case none () (i64.const -1))
            (case some ($result)
              (%match (%result (%tuple (%box i64) i64) i32) (result i64) (%move $result)
                (case err ($e) (i64.const -2))
                (case ok ($tuple)
                  (%match (%tuple (%box i64) i64) (result i64) (%move $tuple)
                    (case pair ($x $n)
                      (%let $f (%closure.new $Take (%move $x))
                        (i64.add (%closure.call $Take (%move $f)) (local.get $n))))))))))))"#),
        "42"
    );
    rejects(
        r#"(module
      (%closure $C (%capture $x (%box i64)) (%box.close (%move $x)))
      (func $f (%let $c (%closure.new $C (%box.new i64 (i64.const 1)))
        (block (%closure.call $C (%move $c)) (%closure.call $C (%move $c))))))"#,
        "already moved",
    );
}

#[test]
fn branches_must_leave_the_same_owners_and_loop_edges_restore_their_contract() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i64)
        (%let $x (%box.new i64 (i64.const 42))
          (%loop $again (bind $n i64 (i64.const 3)) (bind $x (%box i64) (%move $x)) (result i64)
            (if (result i64) (i64.eqz (local.get $n))
              (then (%match (%box i64) (result i64) (%move $x) (case own ($v) (local.get $v))))
              (else (%recur $again (i64.sub (local.get $n) (i64.const 1)) (%move $x))))))))"#),
        "42"
    );
    rejects(
        r#"(module (func $f (param $flag i32)
      (%let $x (%box.new i64 (i64.const 1))
        (block (if (local.get $flag) (then (%box.close (%move $x))) (else (nop)))
          (%box.close (%move $x))))))"#,
        "branch ownership",
    );
    rejects(
        r#"(module (func $f
      (%let $x (%box.new i64 (i64.const 1))
        (%loop $again (bind $n i64 (i64.const 0))
          (block (%box.close (%move $x)) (%recur $again (local.get $n)))))))"#,
        "loop ownership",
    );
}

#[test]
fn owners_cannot_escape_through_shared_storage_or_exception_paths() {
    for source in [
        "(module (type $Box (struct (field (%box i64)))))",
        "(module (type $Array (array (%option (%box i64)))))",
        "(module (type $V (%variant (case (%box i64)))))",
    ] {
        rejects(source, "shared GC storage");
    }
    rejects(
        "(module (func $f (%box.close (%box.new (%box i64) (%box.new i64 (i64.const 1))))))",
        "own payload must be shared",
    );
    rejects("(module (tag $E (param (%box i64))))", "exception payload");
    rejects(
        "(module (tag $E) (func $f (param $x (%box i64)) (throw $E)))",
        "unconsumed owned",
    );
    rejects(
        "(module (import \"host\" \"make\" (func $make (result (%box i64)))))",
        "owned import",
    );
    rejects(
        "(module (func $f (param $x (%box i64)) (unreachable)))",
        "unconsumed owned",
    );
    rejects(
        r#"(module
      (%closure $C (%capture $x (%box i64)) (%box.close (%move $x)))
      (type $Box (struct (field (ref $C)))))"#,
        "shared GC storage",
    );
    rejects(
        r#"(module
      (%closure $C (%capture $x (%box i64)) (%box.close (%move $x)))
      (func $bad (param $c (ref null $C)) (nop)))"#,
        "cannot be nullable",
    );
    rejects(
        r#"(module
      (%closure $C (%capture $x (%box i64)) (%box.close (%move $x)))
      (func $bad (param $a (ref $C)) (param $b (ref $C)) (result i32)
        (ref.eq (%move $a) (%move $b))))"#,
        "identity cannot be inspected",
    );
    rejects(
        "(module (tag $E) (func $f (param $n i64) (%try (do (drop (%move $n))) (catch $E () (nop)))))",
        "move requires an owned",
    );
}

#[test]
fn close_and_return_preserve_evaluation_order_and_shared_values_stay_reusable() {
    assert_eq!(
        run(r#"(module
      (type $Cell (struct (field (mut i64))))
      (func $main (export "main") (result i64)
        (%let $cell (struct.new $Cell (i64.const 0))
          (%let $x (%box.new (ref $Cell) (local.get $cell))
            (block (result i64)
              (struct.set $Cell 0 (local.get $cell) (i64.const 42))
              (%box.close (%move $x))
              (struct.get $Cell 0 (local.get $cell)))))))"#),
        "42"
    );
    rejects(
        "(module (func $f (block (%box.new i64 (i64.const 1)) (nop))))",
        "discard an owned",
    );
}
