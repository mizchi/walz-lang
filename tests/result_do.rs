use std::process::Command;

#[test]
fn result_do_evaluates_once_left_to_right_and_stops_at_the_first_error() {
    for (first, second, expected) in [(0, 0, "42"), (1, 0, "101"), (0, 2, "202")] {
        let source = format!(
            r#"(module
          (%import "wlt:std" (type $result))
          (type $Cell (struct (field $n (mut i32))))
          (func $step (param $c (ref $Cell)) (param $n i32) (param $error i32) (result (%inst $result i32 i32)) (%effect reads writes)
            (block (result (%inst $result i32 i32))
              (struct.set $Cell $n (local.get $c) (i32.add (struct.get $Cell $n (local.get $c)) (i32.const 1)))
              (if (result (%inst $result i32 i32)) (i32.eqz (local.get $error))
                (then (%result.ok (local.get $n))) (else (%result.err (local.get $error))))))
          (func $main (export "main") (result i32) (%effect reads writes allocates)
            (%let $c (struct.new $Cell (i32.const 0))
              (%let $r (%result.do
                  ($a (call $step (local.get $c) (i32.const 40) (i32.const {first})))
                  ($b (call $step (local.get $c) (i32.const 2) (i32.const {second})))
                  (%result.ok (i32.add (local.get $a) (local.get $b))))
                (%match (local.get $r)
                  (case ok ($n) (local.get $n))
                  (case err ($e) (i32.add (i32.mul (local.get $e) (i32.const 100)) (struct.get $Cell $n (local.get $c)))))))))"#
        );
        let compiled = wlt::compile(&source).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("do.wasm");
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
fn result_do_preserves_error_types_unit_payloads_and_ownership() {
    let source = r#"(module
      (%import "wlt:std" (type $result) (type $unit))
      (func $unit (result (%inst $result $unit i32)) (%effect pure) (%result.ok))
      (func $f (result (%inst $result i32 i32)) (%effect pure)
        (%result.do (() (call $unit)) (%result.ok (i32.const 42)))))"#;
    wlt::check(source).unwrap();
    assert!(wlt::check(&source.replace("(() (call $unit))", "($bad (call $unit))")).is_err());
    assert!(
        wlt::check(&source.replace(
            "(%result.ok (i32.const 42))",
            "(%result.err i32 i64 (i64.const 1))"
        ))
        .is_err()
    );
    let owned = r#"(module
      (%import "wlt:std" (type $result) (type $own))
      (func $f (param $x (%inst $own i32)) (param $r (%inst $result i32 i32)) (result (%inst $result i32 i32))
        (%result.do ($a (local.get $r)) (block (result (%inst $result i32 i32)) (%own.close (%move $x)) (%result.ok (local.get $a))))))"#;
    assert!(wlt::check(owned).is_err());
}
