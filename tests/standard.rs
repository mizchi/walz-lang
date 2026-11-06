use std::process::Command;

fn run(source: &str, expected: &str) -> wlt::Compilation {
    let compiled = wlt::compile(source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("standard.wasm");
    std::fs::write(&path, &compiled.wasm).unwrap();
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
    compiled
}

#[test]
fn standard_types_are_explicit_aliases_and_use_normal_instantiation() {
    run(
        r#"(module
      (%import "wlt:std" (type $result $R) (type $option $O) (type $future $F) (type $unit))
      (func $ready (result (%inst $F (%inst $R (%inst $O i64) i32))) (%effects pure)
        (%future.ready (%inst $R (%inst $O i64) i32) (%result.ok (%option.some (i64.const 42)))))
      (func $main (export "main") (result i64) (%effects suspends)
        (%let $f (call $ready)
          (%match (%future.await (%inst $R (%inst $O i64) i32) (%move $f))
            (case ok ($v) (%match (local.get $v) (case none () (i64.const 0)) (case some ($n) (local.get $n))))
            (case err ($e) (i64.const -1))))))"#,
        "42",
    );
    assert!(wlt::check("(module (func $f (param $v (%inst $result i64 i32)) (nop)))").is_err());
}

#[test]
fn standard_stream_pair_list_own_and_scalars_share_existing_checked_types() {
    run(
        r#"(module
      (%import "wlt:std" (type $stream) (type $pair) (type $list) (type $own) (type $text) (type $bytes) (type $unit))
      (%type.alias $Values (%inst $list i64))
      (func $first (param $v $Values) (result i64) (%effects pure)
        (%list.get i64 (local.get $v) (i32.const 0)))
      (func $main (export "main") (result i64)
        (%let (($input (%inst $stream i64) (%stream.empty i64))
               ($owner (%inst $own i64) (%own.new i64 (i64.const 42)))
               ($text $text (%text.lit "hello"))
               ($bytes $bytes (%text.bytes (local.get $text))))
          (block (result i64)
            (%stream.close i64 (%move $input))
            (%match (%move $owner) (case own ($n) (call $first (%list.new i64 (local.get $n)))))))))"#,
        "42",
    );
}

#[test]
fn standard_functions_are_embedded_selective_and_can_be_renamed() {
    let compiled = run(
        r#"(module
      (%import "wlt:std" (type $text) (func $string.u64 $format))
      (func $main (export "main") (result i32) (%effects reads writes allocates)
        (%text.byte-len (call $format (i64.const 42)))))"#,
        "2",
    );
    assert!(!compiled.wat.contains("parse-u64"));
    assert!(!compiled.wat.contains("json"));
    assert!(!compiled.analysis.opaque);
    assert!(
        wlt::check(
            r#"(module (%import "wlt:std" (func $string.u64))
      (func $f (result i64) (call $string.parse-s64 (%text.lit "42"))))"#
        )
        .is_err()
    );
}

#[test]
fn standard_imports_work_from_files_without_resolving_a_std_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    let source = r#"(module (%import "wlt:std" (type $result $Outcome))
      (func $main (export "main") (result i32)
        (%match (%inst $Outcome i32 i32) (result i32) (%result.ok (i32.const 42))
          (case ok ($v) (local.get $v)) (case err ($e) (local.get $e)))))"#;
    std::fs::write(&path, source).unwrap();
    let loaded = wlt::source::load(&path).unwrap();
    run(loaded.text(), "42");
    let native = r#"(module (import "wlt:std" "value" (func $value (result i32))))"#;
    assert_eq!(wlt::compile(native).unwrap().wat, native);
}

#[test]
fn unknown_packages_and_exports_are_source_errors_and_privacy_is_preserved() {
    for (source, code) in [
        (
            r#"(module (%import "wlt:missing" (type $result)))"#,
            "E_STD_MODULE",
        ),
        (
            r#"(module (%import "wlt:std" (type $missing)))"#,
            "E_STD_MEMBER",
        ),
        (
            r#"(module (%import "wlt:std" (func $builder.new) (type $builder.Builder))
          (func $bad (result i32) (struct.get $builder.Builder 1 (call $builder.new))))"#,
            "E_OPAQUE_REPRESENTATION",
        ),
    ] {
        let error = wlt::check(source).unwrap_err();
        assert_eq!(error.diagnostic().code, code, "{error}");
    }
}

#[test]
fn included_units_share_identical_standard_bindings_but_reject_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("library.wlt"),
        r#"(module
      (%import "wlt:std" (type $result))
      (func $make (result (%inst $result i32 i32)) (%result.ok (i32.const 42))))"#,
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        r#"(module (%import "wlt:std" (type $result)) (%include "library.wlt")
      (func $main (export "main") (result i32)
        (%match (call $make) (case ok ($n) (local.get $n)) (case err ($e) (local.get $e)))))"#,
    )
    .unwrap();
    run(wlt::source::load(&main).unwrap().text(), "42");
    let error = wlt::check(r#"(module (%import "wlt:std" (type $result $X) (type $option $X)))"#)
        .unwrap_err();
    assert!(error.to_string().contains("duplicate"));
}

#[test]
fn malformed_standard_aliases_are_diagnostics() {
    for source in [
        r#"(module (%import "wlt:std" (type $result (ref $X))))"#,
        r#"(module (%import "wlt:std" (func $string.u64 ($alias))))"#,
    ] {
        assert_eq!(
            wlt::check(source).unwrap_err().diagnostic().code,
            "E_STD_MEMBER"
        );
    }
}
