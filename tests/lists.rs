#[test]
fn lists_are_shared_values_with_portable_nested_shapes() {
    let source = include_str!("component-fixtures/lists.wlt");
    let compiled = wlt::compile(source).unwrap();
    assert!(compiled.wat.contains("$__list_storage_"));
    let wit = wlt::wit::emit(compiled.ir.as_ref().unwrap()).unwrap();
    assert!(
        wit.contains("list<s64>") && wit.contains("list<string>"),
        "{wit}"
    );
    wlt::component::compile(source).unwrap();
    wlt::component::core_js::compile(source).unwrap();
}

#[test]
fn list_representation_and_owned_elements_are_rejected() {
    for source in [
        "(module (func $f (param $xs (%list (%box i64))) (nop)))",
        "(module (func $f (param $xs (%list (%future i64))) (nop)))",
        "(module (func $f (param $xs (%list (%tuple i32 (%stream i64)))) (nop)))",
        "(module (func $f (param $xs (%list i32)) (result i32) (array.len (local.get $xs))))",
        "(module (func $f (param $xs (%list i32)) (result i32) (ref.eq (local.get $xs) (local.get $xs))))",
        "(module (func $f (param $xs (%list i32)) (result i64) (%list.get i64 (local.get $xs) (i32.const 0))))",
    ] {
        assert!(wlt::check(source).is_err(), "accepted {source}");
    }
}

#[test]
fn abrupt_list_operands_are_bottom_typed_without_panics() {
    for (result, expr) in [
        ("i32", "(%list.len i32 (unreachable))"),
        ("i64", "(%list.get i64 (unreachable) (i32.const 0))"),
        ("(%list i32)", "(%list.new i32 (unreachable))"),
        (
            "(%list i32)",
            "(%list.slice i32 (unreachable) (i32.const 0) (i32.const 0))",
        ),
    ] {
        wlt::compile(&format!("(module (func $f (result {result}) {expr}))")).unwrap();
    }
}

#[test]
fn aliases_nondefaultable_elements_and_list_paths_keep_their_types() {
    let source = r#"(module
      (type $Cell (struct (field (mut i32))))
      (%type.alias $Element (ref $Cell))
      (%type.alias $Cells (%list $Element))
      (func $set (param $xs $Cells) (struct.set $Cell 0 (%list.get $Element (local.get $xs) (i32.const 0)) (i32.const 42)))
      (func $main (export "main") (result i32)
        (%let $cell (struct.new $Cell (i32.const 0))
          (%let $xs (%list.new $Element (local.get $cell))
            (block (result i32)
              (call $set (%list.slice $Element (local.get $xs) (i32.const 0) (i32.const 1)))
              (struct.get $Cell 0 (local.get $cell)))))))"#;
    let compiled = wlt::compile(source).unwrap();
    let report = serde_json::to_value(&compiled.analysis).unwrap();
    let main = report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "$main")
        .unwrap();
    assert_eq!(
        main["memory"]["writes"][0]["object"]["root"]["kind"],
        "allocation"
    );
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("list.wasm");
    std::fs::write(&file, compiled.wasm).unwrap();
    let result = std::process::Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            "main",
        ])
        .arg(file)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
}
