use std::process::Command;

#[test]
fn scalar_newtypes_are_nominal_and_erase_without_gc_allocations() {
    let source = r#"(module
      (type $Id (%newtype i32))
      (type $Count (%newtype i32))
      (func $id (%type.params $T) (%where (shared $T) (nonunit $T)) (param $x $T) (result $T) (%effects pure) (local.get $x))
      (func $take (param $x $Id) (result $Id) (%effects pure) (local.get $x))
      (func $main (export "main") (result i32) (%effects pure)
        (%let $id $Id (%newtype.wrap $Id (i32.const 42))
          (%newtype.unwrap $Id (call $take (call (%inst $id $Id) (local.get $id)))))))"#;
    let module = wlt::check(source).unwrap();
    let report = wlt::typing::report(&module);
    assert!(report.functions.iter().any(|f| f.result == "$Id"));
    let compiled = wlt::compile(source).unwrap();
    assert!(!compiled.analysis.opaque);
    assert!(!compiled.wat.contains("struct.new"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("newtype.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    let output = Command::new("wasmtime")
        .args(["run", "--invoke", "main"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    for value in ["(%newtype.wrap $Count (i32.const 42))", "(i32.const 42)"] {
        let bad = format!(
            "(module (type $Id (%newtype i32)) (type $Count (%newtype i32)) (func $take (param $id $Id) (nop)) (func $bad (call $take {value})))"
        );
        let error = wlt::check(&bad).unwrap_err().to_string();
        assert!(error.contains("$Id"), "{error}");
    }
}

#[test]
fn newtypes_work_in_gc_storage_values_and_portable_scalar_boundaries() {
    let source = r#"(module
      (%import "wlt:std" (type $list))
      (type $Id (%newtype i64))
      (type $Box (struct (field $value $Id)))
      (func $roundtrip (export "roundtrip") (param $id $Id) (result $Id) (%effects pure)
        (local.get $id))
      (func $stored (export "stored") (param $id $Id) (result (%inst $list $Id)) (%effects reads allocates)
        (%let $b (struct.new $Box (local.get $id))
          (%list.new $Id (struct.get $Box $value (local.get $b))))))"#;
    let module = wlt::check(source).unwrap();
    assert!(wlt::wit::emit(&module).unwrap().contains("s64"));
    wlt::component::compile(source).unwrap();
    wlt::component::core_js::compile(source).unwrap();
    for ty in ["v128", "(ref $Box)", "unit"] {
        assert!(
            wlt::check(&source.replace("(%newtype i64)", &format!("(%newtype {ty})"))).is_err()
        );
    }
    assert!(
        wlt::check("(module (type $Id (%newtype i32)) (func $f (param $x (ref $Id)) (nop)))")
            .is_err()
    );
}

#[test]
fn opaque_newtypes_keep_representation_private_and_laws_erase_after_checking() {
    let source = r#"(module
      (%namespace $user
        (%export (opaque $Id) (func $new) (func $number))
        (type $Id (%newtype i32))
        (func $new (param $n i32) (result $Id) (%newtype.wrap $Id (local.get $n)))
        (func $number (param $id $Id) (result i32) (%newtype.unwrap $Id (local.get $id))))
      (%type.alias $Alias $user.Id)
      (func $main (export "main") (result i32) (%effects pure) (call $user.number (call $user.new (i32.const 42)))))"#;
    wlt::compile(source).unwrap();
    for body in [
        "(%newtype.unwrap $user.Id (call $user.new (i32.const 42)))",
        "(call $user.number (%newtype.wrap $user.Id (i32.const 42)))",
    ] {
        let bad = source.replace("(call $user.number (call $user.new (i32.const 42)))", body);
        let error = wlt::check(&bad).unwrap_err();
        assert_eq!(
            error.diagnostic().code,
            "E_OPAQUE_REPRESENTATION",
            "{error}"
        );
    }
    let law = r#"(module (type $Id (%newtype i32))
      (%law "newtype identity" (param $n i32)
        (i32.eq (%newtype.unwrap $Id (%newtype.wrap $Id (local.get $n))) (local.get $n))))"#;
    wlt::verified::compile(law).unwrap();
}

#[test]
fn abrupt_newtype_operands_keep_their_nominal_dependencies_during_linking() {
    for body in [
        "(%newtype.unwrap $Id (unreachable))",
        "(%newtype.unwrap $Id (%newtype.wrap $Id (unreachable)))",
    ] {
        let source = format!(
            "(module (type $Id (%newtype i32)) (func $main (export \"main\") (result i32) (%effects pure) {body}))"
        );
        wlt::compile(&source).unwrap();
    }
}

#[test]
fn erasing_newtypes_does_not_enable_dead_code_elimination_when_disabled() {
    let source = "(module (type $Id (%newtype i32)) (func $unused (result i32) (i32.const 1)) (func $main (export \"main\") (result i32) (%newtype.unwrap $Id (%newtype.wrap $Id (i32.const 42)))))";
    let built = wlt::compile_with_options(source, wlt::CompileOptions { optimize: false }).unwrap();
    assert!(built.wat.contains("$unused"));
}
