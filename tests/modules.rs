#[test]
fn static_namespaces_link_public_functions_types_and_aliases() {
    let source = r#"(module
      (%namespace $math
        (%public (func $add) (type $Box))
        (type $Box (struct (field $n i32)))
        (func $private (param $n i32) (result i32) (local.get $n))
        (func $add (param $add i32) (result (ref $Box))
          (struct.new $Box (call $private (i32.add (local.get $add) (i32.const 2))))))
      (%use $math (func $add $make) (type $Box))
      (func $main (export "main") (result i32)
        (struct.get $Box $n (call $make (i32.const 40)))))"#;
    let compiled = wlt::compile(source).unwrap();
    let ir = compiled.ir.unwrap();
    assert!(ir.functions.iter().any(|f| f.name == "$math.private"));
    assert!(compiled.wat.contains("call $math.add"));
    assert_eq!(ir.types[0].name, "$math.Box");
}

#[test]
fn namespace_visibility_is_checked_before_flattening() {
    for (source, message) in [
        (
            "(module (%namespace $a (func $f (nop))) (func $g (call $a.f)))",
            "private",
        ),
        (
            "(module (%namespace $a (func $f (nop))) (%use $a (func $f)))",
            "private",
        ),
        (
            "(module (%namespace $a (%public (func $missing))))",
            "unknown",
        ),
        (
            "(module (%namespace $a (%public (func $f)) (func $f (nop))) (%use $a (func $f)) (func $f (nop)))",
            "duplicate",
        ),
        (
            "(module (%namespace $a (func $f (export \"leak\") (nop))))",
            "root",
        ),
    ] {
        let error = wlt::check(source).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn nested_modules_and_forward_uses_have_no_runtime_module_objects() {
    let source = "(module (%use $a.b (func $f)) (func $main (export \"main\") (result i32) (call $f)) (%namespace $a (%namespace $b (%public (func $f)) (func $f (result i32) (i32.const 42)))))";
    let compiled = wlt::compile(source).unwrap();
    assert!(compiled.wat.contains("call $a.b.f"));
    assert!(!compiled.wat.contains("namespace"));
}

#[test]
fn file_includes_are_relative_static_and_cycle_checked() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("lib")).unwrap();
    std::fs::write(dir.path().join("lib/math.wlt"), "(module (%namespace $math (%public (func $answer)) (func $answer (result i32) (i32.const 42))))").unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(&main,"(module (%include \"lib/math.wlt\") (%use $math (func $answer)) (func $main (export \"main\") (result i32) (call $answer)) (%test \"included answer\" (i32.eq (call $answer) (i32.const 42))))").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("run")
        .arg(&main)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("test")
        .arg(&main)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    std::fs::write(
        dir.path().join("lib/math.wlt"),
        "(module (%include \"../main.wlt\"))",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("check")
        .arg(&main)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("include cycle"));
    std::fs::write(dir.path().join("lib/math.wlt"),"(module (%namespace $math (%public (func $answer))\n (func $answer (result i32) (i64.const 42))))").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("check")
        .arg(&main)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("math.wlt:2:"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn namespace_resolution_preserves_existing_type_effect_and_ownership_checks() {
    for name in ["variants", "exceptions", "closure", "ownership"] {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("examples/{name}.wlt"));
        let original = std::fs::read_to_string(path)
            .unwrap()
            .replace("(export \"main\")", "")
            .replace("(export \"value\")", "")
            .replace("(export \"divide\")", "");
        let body = original
            .trim()
            .strip_prefix("(module")
            .unwrap()
            .trim_end()
            .strip_suffix(')')
            .unwrap();
        let source = format!(
            "(module (%namespace $lib (%public (func $main)) {body}) (func $entry (export \"main\") (result i64) (call $lib.main)))"
        );
        wlt::compile(&source).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let source = "(module (%namespace $lib (%public (func $set)) (type $C (struct (field (mut i32)))) (func $set (param $c (ref $C)) (struct.set $C 0 (local.get $c) (i32.const 1)))) (func $bad (%effect pure) (call $lib.set (unreachable))))";
    assert!(
        wlt::check(source)
            .unwrap_err()
            .to_string()
            .contains("effect contract")
    );
}

#[test]
fn malformed_module_forms_report_errors_without_panicking() {
    for source in [
        "(module (%use))",
        "(module (%namespace))",
        "(module (%namespace $a) (%use $a))",
        "(module (%namespace $a (%public ())))",
        "(module (%namespace $a (func $f ())))",
    ] {
        assert!(wlt::check(source).is_err(), "{source}");
    }
}

#[test]
fn namespace_value_aliases_qualify_only_type_positions_and_keep_privacy() {
    let source = r#"(module
      (%use $data (type $Count $Count))
      (func $identity (param $Count $Count) (result $Count) (local.get $Count))
      (%namespace $data (%public (type $Count)) (%type.alias $Count i32)))"#;
    wlt::compile(source).unwrap();
    for source in [
        "(module (%namespace $data (%type.alias $Hidden i32)) (func $f (param $n $data.Hidden) (nop)))",
        "(module (%namespace $data (%public (opaque $Count)) (%type.alias $Count i32)))",
        "(module (%type.alias $Same i32) (type $Same (struct)))",
    ] {
        assert!(wlt::check(source).is_err(), "{source}");
    }
}
