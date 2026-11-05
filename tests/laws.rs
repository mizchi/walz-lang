use std::process::Command;

fn command(mode: &str, source: &str) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("law.wlt");
    std::fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg(mode)
        .arg(path)
        .output()
        .unwrap()
}

const GOOD: &str = r#"(module
  (func $add (param $x i32) (param $y i32) (result i32) (%effects pure)
    (i32.add (local.get $x) (local.get $y)))
  (%law "addition is commutative" (param $x i32) (param $y i32)
    (i32.eq (call $add (local.get $x) (local.get $y))
            (call $add (local.get $y) (local.get $x))))
  (%law "wrapping is wasm wrapping" (param $x i64)
    (i64.eq (i64.add (local.get $x) (i64.const 1))
            (i64.sub (local.get $x) (i64.const -1))))
  (func $main (export "main") (result i32) (%effects pure)
    (call $add (i32.const 40) (i32.const 2))))"#;

#[test]
fn laws_are_universally_proved_and_run_through_the_same_gate_in_normal_run() {
    let output = command("verify", GOOD);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = command("run", GOOD);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    let output = command("proof", GOOD);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let proof = String::from_utf8(output.stdout).unwrap();
    assert!(proof.contains("BitVec 32") && proof.contains("bv_decide"));
}

#[test]
fn false_laws_block_execution_and_unsupported_semantics_are_rejected() {
    for mode in ["run", "run-verified", "verify", "wat", "check"] {
        let false_law = GOOD.replace(
            "(i32.add (local.get $x) (local.get $y))",
            "(i32.sub (local.get $x) (local.get $y))",
        );
        let output = command(mode, &false_law);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("42"));
    }
    for (source, expected) in [
        (
            "(module (%law \"not a bounded test\" (param $x i32) (i32.ne (local.get $x) (i32.const 65537))))",
            "contract kernel",
        ),
        (
            "(module (%law \"division\" (param $x i32) (i32.eqz (i32.div_s (local.get $x) (i32.const 1)))))",
            "unsupported law operation",
        ),
        (
            "(module (func $f (param $x i32) (result i32) (call $f (local.get $x))) (%law \"recursive\" (param $x i32) (call $f (local.get $x))))",
            "recursive law",
        ),
        (
            "(module (%law \"bad return type\" (i64.const 1)))",
            "expected i32",
        ),
        (
            "(module (%law \"duplicate\" (i32.const 1)) (%law \"duplicate\" (i32.const 1)))",
            "duplicate law",
        ),
    ] {
        let output = command("verify", source);
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn proof_semantics_preserve_signedness_wrapping_masked_shifts_and_branching() {
    let source = r#"(module
      (%law "constant" (i32.const 1))
      (%law "signed comparison" (i32.lt_s (i32.const -1) (i32.const 0)))
      (%law "unsigned comparison" (i32.gt_u (i32.const -1) (i32.const 0)))
      (%law "sign extension" (i64.eq (i64.extend_i32_s (i32.const -1)) (i64.const -1)))
      (%law "zero extension" (i64.eq (i64.extend_i32_u (i32.const -1)) (i64.const 4294967295)))
      (%law "masked left shift" (param $x i32) (param $y i32)
        (i32.eq (i32.shl (local.get $x) (local.get $y))
          (i32.shl (local.get $x) (i32.and (local.get $y) (i32.const 31)))))
      (%law "logical right shift" (i32.eq (i32.shr_u (i32.const -1) (i32.const 33)) (i32.const 2147483647)))
      (%law "arithmetic right shift" (i32.eq (i32.shr_s (i32.const -1) (i32.const 33)) (i32.const -1)))
      (%law "hex wrapping" (i64.eq (i64.add (i64.const 0xffff_ffff_ffff_ffff) (i64.const 1)) (i64.const 0)))
      (%law "nonzero after conditional" (param $x i32)
        (%let $y (if (result i32) (i32.eqz (local.get $x))
                  (then (i32.const 1)) (else (local.get $x)))
          (i32.ne (local.get $y) (i32.const 0))))
      (func $main (export "main") (result i32) (%effects pure)
        (i32.add
          (i32.shr_u (i32.const -1) (i32.const 33))
          (i32.const -2147483605))))"#;
    let output = command("run-verified", source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}
