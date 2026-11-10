const SOURCE: &str = r#"(module
  (memory $data 1)
  (func $main (export "main") (result i32) (@effect reads writes)
    (@let $pointer (type i32) 16
      (block (result i32)
        (memory.fill $data (i32.const 0) (i32.const 0) (i32.const 64))
        (i32.store $data $pointer (i32.const 42))
        (memory.copy $data $data (i32.const 32) $pointer (i32.const 4))
        (i32.load $data (i32.const 32))))))"#;

#[test]
fn wat_memory_instructions_and_typed_macros_share_validator_types() {
    let checked = walz::check(SOURCE).unwrap();
    let analysis = walz::analysis::analyze(&checked);
    assert!(!analysis.opaque);
    assert!(
        analysis.functions[0].effects.reads
            && analysis.functions[0].effects.writes
            && analysis.functions[0].effects.may_trap
    );
    walz::compile(SOURCE).unwrap();
    walz::component::core_js::compile(SOURCE).unwrap();
    walz::component::compile(SOURCE).unwrap();
    let bad = SOURCE.replace("(i32.const 42)", "(i64.const 42)");
    assert!(walz::check(&bad).is_err());
    let bad = SOURCE.replace("(@effect reads writes)", "(@effect pure)");
    assert!(walz::check(&bad).is_err());
}

#[test]
fn raw_memory_cannot_bypass_race_contracts_or_owned_values() {
    let source = SOURCE.replace("(memory $data 1)", "(@race-free) (memory $data 1 1 shared)");
    assert!(walz::compile(&source).is_err());
    let source = r#"(module (@use "walz:core" (type $box)) (memory $m 1)
      (func $bad (param $value (@inst $box i32)) (i32.store $m (i32.const 0) (@move $value))))"#;
    assert!(walz::check(source).is_err());
    let invalid =
        "(module (memory $m 1) (func $f (result i32) (i32.load $m align=8 (i32.const 0))))";
    assert!(walz::check(invalid).is_err());
}

#[test]
fn file_modules_keep_named_memory_dependencies_through_dce() {
    let dir = tempfile::tempdir().unwrap();
    let library = SOURCE.replace(
        "(func $main (export \"main\")",
        "(@export (func $main)) (func $main",
    );
    std::fs::write(dir.path().join("memory.walz"), library).unwrap();
    let main = dir.path().join("main.walz");
    std::fs::write(&main,"(module (@use \"./memory.walz\" (func $main $work)) (func $main (export \"main\") (result i32) (@effect reads writes) (call $work)))").unwrap();
    let source = walz::source::load(main).unwrap();
    let compiled = walz::compile(source.text()).unwrap();
    assert_eq!(compiled.ir.unwrap().memories.len(), 1);
}
