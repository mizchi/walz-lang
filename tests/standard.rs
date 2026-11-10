use std::process::Command;

#[test]
fn core_is_the_canonical_builtin_package_and_std_is_not_a_public_alias() {
    let source = r#"(module (@use "walz:core" (type $result))
      (func $main (export "main") (result i64)
        (@match (@result.ok i64 i32 42)
          (@case ok ($value) $value) (@case err ($code) 0))))"#;
    run(source, "42");
    let old = source.replace("walz:core", "walz:std");
    let error = walz::check(&old).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_CORE_MODULE");
    assert!(error.to_string().contains("walz:core"));
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("main.walz");
    std::fs::write(&file, &old).unwrap();
    let loaded = walz::source::load_legacy(&file).unwrap();
    assert!(loaded.text().contains("walz:core"));
    run(loaded.text(), "42");
    let native = r#"(module (import "walz:std" "value" (func $value (result i32))))"#;
    assert_eq!(walz::compile(native).unwrap().wat, native);
}

#[test]
fn legacy_package_conversion_preserves_wasm_imports_literals_and_source_positions() {
    let source = r#"(module
      (@import "walz:std" (type $string))
      (import "walz:std" "literal" (func $host))
      (func $main (result $string) (@effect pure)
        (block (result $string) (drop (@string.lit "walz:std")) $missing)))"#;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.walz");
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load_legacy(&path).unwrap();
    assert!(loaded.text().contains("\"walz:core\""));
    assert_eq!(loaded.text().matches("\"walz:std\"").count(), 2);
    assert!(loaded.text().contains("(import \"walz:std\" \"literal\""));
    let error = loaded.diagnostic(walz::check(loaded.text()).unwrap_err());
    assert_eq!(
        error.diagnostic().primary.as_ref().unwrap().span.start,
        source.find("$missing").unwrap()
    );
}

fn run(source: &str, expected: &str) -> walz::Compilation {
    let compiled = walz::compile(source).unwrap();
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
      (@use "walz:core" (type $result $R) (type $option $O) (type $future $F) (type $unit))
      (func $ready (result (@inst $F (@inst $R (@inst $O i64) i32))) (@effect pure)
        (@future.ready (@inst $R (@inst $O i64) i32) (@result.ok (@option.some (i64.const 42)))))
      (func $main (export "main") (result i64) (@effect suspends)
        (@let $f (call $ready)
          (@match (@future.await (@inst $R (@inst $O i64) i32) (@move $f))
            (@case ok ($v) (@match (local.get $v) (@case none () (i64.const 0)) (@case some ($n) (local.get $n))))
            (@case err ($e) (i64.const -1))))))"#,
        "42",
    );
    assert!(walz::check("(module (func $f (param $v (@inst $result i64 i32)) (nop)))").is_err());
}

#[test]
fn string_is_the_canonical_standard_alias_for_utf8_text() {
    run(
        r#"(module (@use "walz:core" (type $string))
          (func $main (export "main") (result i32)
            (@let $s (type $string) (@string.lit "é") (@string.byte-len $s))))"#,
        "2",
    );
    assert_eq!(
        walz::check(r#"(module (@use "walz:core" (type $text)))"#)
            .unwrap_err()
            .diagnostic()
            .code,
        "E_CORE_MEMBER"
    );
}

#[test]
fn frozen_text_imports_keep_local_names_and_original_diagnostic_offsets() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("historical.walz");
    for binding in ["(type $text)", "(type $text $S)"] {
        let local = if binding.ends_with("$S)") {
            "$S"
        } else {
            "$text"
        };
        let original = format!(
            "(module ;; $text remains verbatim here\n  (@use \"walz:core\" {binding})\n  (func $f (param $s {local}) (result {local}) (@effects pure) $missing))"
        );
        std::fs::write(&path, &original).unwrap();
        let loaded = walz::source::load_legacy(&path).unwrap();
        assert!(loaded.text().contains("(type $string "));
        assert!(loaded.text().contains(";; $text remains verbatim here"));
        let error = loaded.diagnostic(walz::check(loaded.text()).unwrap_err());
        assert_eq!(error.diagnostic().code, "E_UNKNOWN_LOCAL", "{error}");
        let label = error.diagnostic().primary.as_ref().unwrap();
        assert_eq!(label.location.line, 3);
        assert_eq!(
            label.location.column,
            original.lines().nth(2).unwrap().find("$missing").unwrap() + 1
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        walz::compile(&loaded.text().replace("$missing", "$s")).unwrap();
    }
}

#[test]
fn standard_stream_pair_list_own_and_scalars_share_existing_checked_types() {
    run(
        r#"(module
      (@use "walz:core" (type $stream) (type $tuple) (type $list) (type $box) (type $string) (type $bytes) (type $unit))
      (@type.alias $Values (@inst $list i64))
      (func $first (param $v $Values) (result i64) (@effect pure)
        (@list.get i64 (local.get $v) (i32.const 0)))
      (func $main (export "main") (result i64)
        (@let (($input (@inst $stream i64) (@stream.empty i64))
               ($owner (@inst $box i64) (@box.new i64 (i64.const 42)))
               ($string $string (@string.lit "hello"))
               ($bytes $bytes (@string.bytes (local.get $string))))
          (block (result i64)
            (@stream.close i64 (@move $input))
            (@match (@move $owner) (@case own ($n) (call $first (@list.new i64 (local.get $n)))))))))"#,
        "42",
    );
}

#[test]
fn standard_functions_are_embedded_selective_and_can_be_renamed() {
    let compiled = run(
        r#"(module
      (@use "walz:core" (type $string) (func $string.u64 $format))
      (func $main (export "main") (result i32) (@effect reads writes allocates)
        (@string.byte-len (call $format (i64.const 42)))))"#,
        "2",
    );
    assert!(!compiled.wat.contains("parse-u64"));
    assert!(!compiled.wat.contains("json"));
    assert!(!compiled.analysis.opaque);
    assert!(
        walz::check(
            r#"(module (@use "walz:core" (func $string.u64))
      (func $f (result i64) (call $string.parse-s64 (@string.lit "42"))))"#
        )
        .is_err()
    );
}

#[test]
fn standard_imports_work_from_files_without_resolving_a_std_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.walz");
    let source = r#"(module (@use "walz:core" (type $result $Outcome))
      (func $main (export "main") (result i32)
        (@match (@inst $Outcome i32 i32) (result i32) (@result.ok (i32.const 42))
          (@case ok ($v) (local.get $v)) (@case err ($e) (local.get $e)))))"#;
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load(&path).unwrap();
    run(loaded.text(), "42");
    let native = r#"(module (import "walz:core" "value" (func $value (result i32))))"#;
    assert_eq!(walz::compile(native).unwrap().wat, native);
}

#[test]
fn unknown_packages_and_exports_are_source_errors_and_privacy_is_preserved() {
    for (source, code) in [
        (
            r#"(module (@use "walz:missing" (type $result)))"#,
            "E_CORE_MODULE",
        ),
        (
            r#"(module (@use "walz:core" (type $missing)))"#,
            "E_CORE_MEMBER",
        ),
        (
            r#"(module (@use "walz:core" (func $string-builder.new) (type $string-builder))
          (func $bad (result i32) (struct.get $string-builder 1 (call $string-builder.new))))"#,
            "E_OPAQUE_REPRESENTATION",
        ),
    ] {
        let error = walz::check(source).unwrap_err();
        assert_eq!(error.diagnostic().code, code, "{error}");
    }
}

#[test]
fn included_units_share_identical_standard_bindings_but_reject_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("library.walz"),
        r#"(module
      (@use "walz:core" (type $result))
      (func $make (result (@inst $result i32 i32)) (@result.ok (i32.const 42))))"#,
    )
    .unwrap();
    let main = dir.path().join("main.walz");
    std::fs::write(
        &main,
        r#"(module (@use "walz:core" (type $result)) (@include "library.walz")
      (func $main (export "main") (result i32)
        (@match (call $make) (@case ok ($n) (local.get $n)) (@case err ($e) (local.get $e)))))"#,
    )
    .unwrap();
    run(walz::source::load(&main).unwrap().text(), "42");
    let error = walz::check(r#"(module (@use "walz:core" (type $result $X) (type $option $X)))"#)
        .unwrap_err();
    assert!(error.to_string().contains("duplicate"));
}

#[test]
fn malformed_standard_aliases_are_diagnostics() {
    for source in [
        r#"(module (@use "walz:core" (type $result (ref $X))))"#,
        r#"(module (@use "walz:core" (func $string.u64 ($alias))))"#,
    ] {
        assert_eq!(
            walz::check(source).unwrap_err().diagnostic().code,
            "E_CORE_MEMBER"
        );
    }
}
