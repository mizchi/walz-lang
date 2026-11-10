use std::process::Command;

fn validate_wit(source: &str) -> String {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.walz");
    std::fs::write(&input, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("wit")
        .arg(input)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wit = String::from_utf8(output.stdout).unwrap();
    let path = directory.path().join("generated.wit");
    std::fs::write(&path, &wit).unwrap();
    let binary = directory.path().join("contract.wasm");
    let output = Command::new("wasm-tools")
        .args(["component", "wit"])
        .arg(path)
        .args(["--wasm", "-o"])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{wit}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(binary).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    wit
}

#[test]
fn emits_wit_for_nested_options_results_and_unit_payloads() {
    let wit = validate_wit(
        r#"(module
      (func $lookup (export "lookup") (param $key i64) (result (@option (@result i64 i32)))
        (@effect pure) (@option.some (@result i64 i32) (@result.ok i64 i32 (local.get $key))))
      (func $done (export "done") (result (@result @unit i32)) (@effect pure) (@result.ok @unit i32))
      (func $empty (export "type") (result (@result i64 @unit)) (@effect pure) (@result.err i64 @unit))
      (func $both (export "walz-unit") (result (@result @unit @unit)) (@effect pure) (@result.ok @unit @unit))
      (func $event (export "event") (result (@option @unit)) (@effect pure) (@option.some @unit))
      (func $tuple (export "pair") (result (@tuple @unit i64)) (@effect pure) (@tuple.new @unit i64 (i64.const 42))))"#,
    );
    assert!(wit.contains("option<result<s64, s32>>"), "{wit}");
    assert!(wit.contains("result<_, s32>"), "{wit}");
    assert!(
        wit.contains("enum") && wit.contains("option<%walz-unit-a1>"),
        "{wit}"
    );
    assert!(
        wit.contains("%type: func() -> result<s64>;")
            && wit.contains("%walz-unit: func() -> result;"),
        "{wit}"
    );
    assert!(wit.contains("tuple<%walz-unit-a1, s64>"), "{wit}");
}

#[test]
fn imported_suspension_becomes_async_only_after_escaping_errors_are_handled() {
    let wit = validate_wit(
        r#"(module
      (import "host" "next" (func $next (result i64)))
      (func $read (export "read") (result i64)
        (@try (result i64) (do (call $next)) (catch_all (i64.const -1)))))"#,
    );
    assert!(wit.contains("async func"), "{wit}");
}

#[test]
fn delivery_types_preserve_wasi_shapes_and_reads_have_async_signatures() {
    let wit = validate_wit(
        r#"(module
      (func $fetch (export "fetch") (result (@tuple (@stream i64) (@future (@result @unit i32)))) (@effect pure)
        (@tuple.new (@stream i64) (@future (@result @unit i32))
          (@stream.empty i64) (@future.ready (@result @unit i32) (@result.ok @unit i32))))
      (func $read (export "read") (param $next (@future i64)) (result i64) (@effect suspends)
        (@future.await i64 (@move $next)))
      (func $step (export "step") (param $stream (@stream i64)) (result (@tuple (@stream i64) (@option i64))) (@effect suspends)
        (@stream.next i64 (@move $stream)))
      (func $event (export "event") (result (@future @unit)) (@effect pure) (@future.ready @unit))
      (func $events (export "events") (result (@stream @unit)) (@effect pure) (@stream.empty @unit)))"#,
    );
    assert!(
        wit.contains("tuple<stream<s64>, future<result<_, s32>>>"),
        "{wit}"
    );
    assert!(wit.contains("%fetch: func()"), "{wit}");
    assert!(
        wit.contains("%read: async func(p0: future<s64>) -> s64"),
        "{wit}"
    );
    assert!(
        wit.contains("%step: async func(p0: stream<s64>) -> tuple<stream<s64>, option<s64>>"),
        "{wit}"
    );
    assert!(
        wit.contains("%event: func() -> future;") && wit.contains("%events: func() -> stream;"),
        "{wit}"
    );
}

#[test]
fn rejects_nonportable_types_and_escaping_exceptions() {
    for (source, expected) in [
        (
            "(module (type $C (struct)) (func $f (export \"f\") (result (ref $C)) (struct.new $C)))",
            "GC references are not WIT value types",
        ),
        (
            "(module (tag $E) (func $f (export \"f\") (throw $E)))",
            "unhandled exceptions",
        ),
        (
            "(module (func $f (export \"Bad_Name\") (nop)))",
            "WIT export name",
        ),
        (
            "(module (func $f (export \"f\") (result (@box i64)) (@box.new i64 (i64.const 1))))",
            "explicit resource adapter",
        ),
        ("(module (func $f (nop)))", "no exported functions"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("bad.walz");
        std::fs::write(&input, source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_walz"))
            .arg("wit")
            .arg(input)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
