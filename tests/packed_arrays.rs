#[test]
fn packed_gc_arrays_and_bulk_byte_snapshots_are_checked_and_do_not_need_linear_memory() {
    let source = walz::source::load("tests/component-fixtures/packed-arrays.walz").unwrap();
    let compiled = walz::verified::compile(source.text()).unwrap();
    assert!(compiled.ir.is_some());
    assert!(compiled.wat.contains("(array (mut i8))"));
    assert!(compiled.wat.contains("array.copy"));
    assert!(!compiled.wat.contains("(memory "));
    walz::component::compile(source.text()).unwrap();
}

#[test]
fn packed_access_and_copy_storage_mutability_and_effects_are_enforced() {
    for source in [
        "(module (type $A (array (mut i8))) (func $f (param $a (ref $A)) (result i32) (array.get $A $a 0)))",
        "(module (type $A (array (mut i32))) (func $f (param $a (ref $A)) (result i32) (array.get_u $A $a 0)))",
        "(module (type $A (array i8)) (func $f (param $a (ref $A)) (array.copy $A $A $a 0 $a 0 0)))",
        "(module (type $A (array (mut i8))) (type $B (array (mut i32))) (func $f (param $a (ref $A)) (param $b (ref $B)) (array.copy $A $B $a 0 $b 0 0)))",
        "(module (type $A (array (mut i8))) (func $f (param $a (ref $A)) (@effect pure) (array.copy $A $A $a 0 $a 0 0)))",
        "(module (@use \"walz:core\" (type $bytes)) (type $A (array i8)) (func $f (param $a (ref $A)) (param $b $bytes) (@bytes.copy-to-array $A $a 0 $b 0 0)))",
        "(module (@use \"walz:core\" (type $bytes)) (type $A (array (mut i8))) (func $f (param $a (ref $A)) (param $b $bytes) (@effect pure) (@bytes.copy-to-array $A $a 0 $b 0 0)))",
    ] {
        assert!(walz::check(source).is_err(), "{source}");
    }
}

#[test]
fn bulk_operations_do_not_expose_opaque_array_storage() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("secret.walz"),
        "(module (@export (opaque $Secret)) (type $Secret (array (mut i8))))",
    )
    .unwrap();
    for operation in [
        "(array.copy $Secret $Public $secret 0 $public 0 0)",
        "(array.copy $Public $Secret $public 0 $secret 0 0)",
        "(@bytes.copy-to-array $Secret $secret 0 (@bytes.new) 0 0)",
    ] {
        let path = directory.path().join("main.walz");
        std::fs::write(&path, format!("(module (@use \"walz:core\" (type $bytes)) (@use \"./secret.walz\" (type $Secret)) (type $Public (array (mut i8))) (func $f (param $secret (ref $Secret)) (param $public (ref $Public)) {operation}))")).unwrap();
        let loaded = walz::source::load(&path).unwrap();
        assert_eq!(
            walz::check(loaded.text()).unwrap_err().diagnostic().code,
            "E_OPAQUE_REPRESENTATION"
        );
    }
}

#[test]
fn imported_macros_keep_packed_storage_in_the_definition_type_namespace() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("syntax.walz"),
        r#"(module
      (@use "walz:core" (type $bytes))
      (type $A (array (mut i8))) (type $B (array (mut i8)))
      (@macro $snapshot (rule (?x:expr)
        (@let (($a (array.new $A 0 1)) ($b (array.new $B ?x 1)))
          (block (result $bytes)
            (array.copy $A $B $a 0 $b 0 1)
            (@bytes.copy-to-array $A $a 0 (@bytes.from-array $B $b) 0 1)
            (@bytes.new (array.get_u $A $a 0))))))
      (@export (macro $snapshot)))"#,
    )
    .unwrap();
    let path = directory.path().join("main.walz");
    std::fs::write(
        &path,
        r#"(module (@use "walz:core" (type $bytes))
      (@use "./syntax.walz" (macro $snapshot))
      (type $A (array (mut i64))) (type $B (array (mut i64)))
      (func $main (export "main") (result $bytes) (@effect reads writes allocates) (@$snapshot 255)))"#,
    )
    .unwrap();
    let source = walz::source::load(&path).unwrap();
    let compiled = walz::verified::compile(source.text()).unwrap();
    assert!(compiled.wat.contains("array.get_u"));
}
