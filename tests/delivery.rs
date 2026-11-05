use std::process::Command;

fn run(source: &str) -> String {
    let compilation = wlt::compile(source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("delivery.wasm");
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

fn rejects(source: &str, message: &str) {
    let error = wlt::check(source).unwrap_err().to_string();
    assert!(error.contains(message), "{source}\n{error}");
}

#[test]
fn ready_future_is_consumed_by_await_and_supports_result_and_unit() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i64) (effects suspends)
        (let $done (future.ready (result i64 i32) (result.ok i64 i32 (i64.const 42)))
          (block (result i64)
            (future.await unit (future.ready unit))
            (future.close unit (future.ready unit))
            (match (result i64 i32) (result i64) (future.await (result i64 i32) (move $done))
              (case ok ($value) (local.get $value)) (case err ($error) (i64.const -1)))))))"#),
        "42"
    );
    rejects(
        r#"(module (func $f (effects suspends)
      (let $f (future.ready i64 (i64.const 1))
        (block (drop (future.await i64 (move $f))) (future.close i64 (move $f))))))"#,
        "already moved",
    );
}

#[test]
fn finite_stream_preserves_order_and_returns_an_owner_even_at_end() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i64) (effects suspends)
        (loop $read
          (bind $stream (stream i64)
            (stream.cons i64 (i64.const 4) (stream.cons i64 (i64.const 2) (stream.empty i64))))
          (bind $sum i64 (i64.const 0)) (result i64)
          (match (pair (stream i64) (option i64)) (result i64) (stream.next i64 (move $stream))
            (case pair ($next $item)
              (match (option i64) (result i64) (local.get $item)
                (case none () (block (result i64) (stream.close i64 (move $next)) (local.get $sum)))
                (case some ($value) (recur $read (move $next)
                  (i64.add (i64.mul (local.get $sum) (i64.const 10)) (local.get $value))))))))))"#),
        "42"
    );
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i32) (effects suspends)
        (match (pair (stream unit) (option unit)) (result i32)
          (stream.next unit (stream.cons unit (stream.empty unit)))
          (case pair ($next $event)
            (match (pair (stream unit) (option unit)) (result i32) (stream.next unit (move $next))
              (case pair ($end $none)
                (block (result i32)
                  (stream.close unit (move $end))
                  (i32.add
                    (match (option unit) (result i32) (local.get $event)
                      (case none () (i32.const 0)) (case some () (i32.const 40)))
                    (match (option unit) (result i32) (local.get $none)
                      (case none () (i32.const 2)) (case some () (i32.const 0)))))))))))"#),
        "42"
    );
}

#[test]
fn stream_end_and_completion_error_remain_separate_values() {
    assert_eq!(
        run(r#"(module
      (func $main (export "main") (result i32) (effects suspends)
        (let $io
          (pair.new (stream i64) (future (result unit i32))
            (stream.empty i64) (future.ready (result unit i32) (result.err unit i32 (i32.const 42))))
          (match (pair (stream i64) (future (result unit i32))) (result i32) (move $io)
            (case pair ($stream $completion)
              (match (pair (stream i64) (option i64)) (result i32) (stream.next i64 (move $stream))
                (case pair ($end $item)
                  (block (result i32)
                    (stream.close i64 (move $end))
                    (match (result unit i32) (result i32)
                      (future.await (result unit i32) (move $completion))
                      (case ok () (i32.const 0)) (case err ($error) (local.get $error)))))))))))"#),
        "42"
    );
}

#[test]
fn endpoints_are_opaque_owned_values_and_payloads_are_shared_for_now() {
    for (source, message) in [
        (
            "(module (func $f (let $f (future.ready unit) (future.close unit (local.get $f)))))",
            "explicit move",
        ),
        (
            "(module (func $f (let $s (stream.empty i64) (nop))))",
            "unconsumed owned",
        ),
        (
            "(module (type $Box (struct (field (stream i64)))))",
            "shared GC storage",
        ),
        (
            "(module (func $f (future.close (own i64) (future.ready (own i64) (own.new i64 (i64.const 1))))))",
            "delivery payload must be shared",
        ),
        (
            "(module (func $f (stream.close (future unit) (stream.empty (future unit)))))",
            "delivery payload must be shared",
        ),
        (
            "(module (func $f (match (future i64) (future.ready i64 (i64.const 1)) (case ready ($n) (nop)))))",
            "opaque delivery",
        ),
        (
            "(module (func $f (effects suspends) (drop (future.await i32 (future.ready i64 (i64.const 1))))))",
            "type mismatch",
        ),
        (
            "(module (func $f (future.close unit (future.ready unit (i32.const 1)))))",
            "argument count",
        ),
        (
            "(module (func $f (stream.close i64 (stream.cons i64 (i64.const 1) (stream.empty i32)))))",
            "type mismatch",
        ),
        (
            "(module (func $f (effects suspends) (drop (stream.next i64 (stream.empty i64)))))",
            "cannot drop an owned",
        ),
    ] {
        rejects(source, message);
    }
}

#[test]
fn suspension_is_checked_and_propagates_without_hiding_payload_effects() {
    rejects(
        "(module (func $f (effects pure) (future.await unit (future.ready unit))))",
        "suspends",
    );
    let source = r#"(module
      (func $read (result i64) (effects suspends) (future.await i64 (future.ready i64 (i64.const 42))))
      (func $call (result i64) (effects suspends) (call $read)))"#;
    let info = wlt::compile(source).unwrap();
    for function in &info.analysis.functions {
        assert!(function.effects.may_suspend);
        assert!(function.semantic_effects.suspends);
        assert!(function.effects.allocates);
        assert!(!function.semantic_effects.host);
    }
    rejects(
        r#"(module (type $Cell (struct (field (mut i64))))
      (func $f (param $cell (ref $Cell)) (effects suspends)
        (future.close unit (future.ready unit
          (struct.set $Cell 0 (local.get $cell) (i64.const 1))))))"#,
        "argument count",
    );
    rejects(
        r#"(module (type $Cell (struct (field (mut i64))))
      (func $f (param $cell (ref $Cell)) (effects pure)
        (future.close i64 (future.ready i64
          (block (result i64) (struct.set $Cell 0 (local.get $cell) (i64.const 1)) (i64.const 42))))))"#,
        "writes",
    );
}

#[test]
fn delivery_evaluates_arguments_once_and_preserves_gc_payload_references() {
    assert_eq!(
        run(r#"(module
      (type $Cell (struct (field (mut i64))))
      (func $mark (param $cell (ref $Cell)) (param $digit i64) (result i64)
        (block (result i64)
          (struct.set $Cell 0 (local.get $cell)
            (i64.add (i64.mul (struct.get $Cell 0 (local.get $cell)) (i64.const 10)) (local.get $digit)))
          (local.get $digit)))
      (func $ready (param $cell (ref $Cell)) (result (future (ref $Cell)))
        (block (result (future (ref $Cell)))
          (struct.set $Cell 0 (local.get $cell)
            (i64.add (struct.get $Cell 0 (local.get $cell)) (i64.const 1)))
          (future.ready (ref $Cell) (local.get $cell))))
      (func $main (export "main") (result i64)
        (let $cell (struct.new $Cell (i64.const 0))
          (let $stream
            (stream.cons i64 (call $mark (local.get $cell) (i64.const 4))
              (stream.cons i64 (call $mark (local.get $cell) (i64.const 1)) (stream.empty i64)))
            (block (result i64)
              (stream.close i64 (move $stream))
              (struct.get $Cell 0 (future.await (ref $Cell) (call $ready (local.get $cell)))))))))"#),
        "42"
    );
}
