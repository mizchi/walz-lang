use std::process::Command;

#[test]
fn scoped_result_macros_have_executable_cleanup_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "tests/scoped-macros.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("8 passed; 0 failed; 0 errors"));
}

#[test]
fn result_propagation_does_not_hide_unconsumed_owners_or_effects() {
    let source = r#"(module (func $f (result (%result i64 i32))
      (%let $owner (%future.ready i64 (i64.const 0))
        (%result.let ($v) (%result i64 i32) i64 (%result.err i64 i32 (i32.const 1))
          (block (result (%result i64 i32)) (%future.close i64 (%move $owner)) (%result.ok i64 i32 (local.get $v)))))))"#;
    let error = wlt::check(source).unwrap_err().to_string();
    assert!(error.contains("ownership"), "{error}");
    let source = "(module (func $f (result (%result i64 i32)) (%effects pure) (%stream.fold-result i64 i64 i32 (%stream.empty (%result i64 i32)) (i64.const 0) ($s $i) (%result.ok i64 i32 (local.get $s)))))";
    assert!(
        wlt::check(source)
            .unwrap_err()
            .to_string()
            .contains("suspends")
    );
}
