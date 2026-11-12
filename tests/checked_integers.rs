#[test]
fn checked_integer_helpers_are_explicit_core_imports_with_pure_typed_contracts() {
    for (scalar, operation) in [
        ("i32", "add"),
        ("i32", "mul"),
        ("i32", "align-up"),
        ("i64", "add"),
        ("i64", "mul"),
        ("i64", "align-up"),
    ] {
        let source = format!(
            r#"(module
          (@use "walz:core" (type $option) (func ${scalar}.checked-{operation}-u $checked))
          (func $f (export "f") (param $a {scalar}) (param $b {scalar})
            (result (@inst $option {scalar})) (@effect pure) (call $checked $a $b)))"#
        );
        let compiled = walz::compile(&source).unwrap();
        assert!(compiled.ir.is_some());
        assert!(!compiled.wat.contains("(memory "));
        assert!(!compiled.wat.contains("(import "));
    }
    let fixture = walz::source::load("tests/component-fixtures/checked-integers.walz").unwrap();
    walz::verified::compile(fixture.text()).unwrap();
    walz::component::compile(fixture.text()).unwrap();
}

#[test]
fn integer_widths_explicit_imports_effects_and_ownership_are_preserved() {
    for (params, result, args) in [
        ("(param $a i32)", "i64", "$a 1"),
        ("(param $a i64)", "i32", "$a 1"),
        ("(param $a f32)", "i32", "$a 1"),
    ] {
        let source = format!(
            r#"(module
          (@use "walz:core" (type $option) (func $i32.checked-add-u))
          (func $bad {params} (result (@inst $option {result}))
            (call $i32.checked-add-u {args})))"#
        );
        assert!(walz::check(&source).is_err(), "{source}");
    }
    assert!(
        walz::check(
            r#"(module (@use "walz:core" (type $option))
      (func $bad (result (@inst $option i32)) (call $i32.checked-add-u 1 2)))"#
        )
        .is_err()
    );
    assert!(
        walz::check(
            r#"(module
      (@use "walz:core" (type $option) (type $box) (func $i32.checked-add-u))
      (func $bad (param $x (@inst $box i32)) (result (@inst $option i32))
        (call $i32.checked-add-u (@move $x) 0)))"#
        )
        .is_err()
    );
    let error = walz::check(
        r#"(module
      (@use "walz:core" (type $option) (func $i32.checked-add-u))
      (type $A (array (mut i32)))
      (func $bad (param $a (ref $A)) (result (@inst $option i32)) (@effect pure)
        (call $i32.checked-add-u (array.get $A $a 0) 0)))"#,
    )
    .unwrap_err();
    assert_eq!(error.diagnostic().code, "E_EFFECT_CONTRACT");
    let fixture = walz::source::load("tests/component-fixtures/checked-integers.walz").unwrap();
    let module = walz::check(fixture.text()).unwrap();
    for function in walz::analysis::analyze(&module)
        .functions
        .iter()
        .filter(|f| f.name.contains(".checked-"))
    {
        let effects = &function.semantic_effects;
        assert!(
            !effects.reads
                && !effects.writes
                && !effects.host
                && !effects.suspends
                && effects.throws.is_empty()
        );
    }
    let watz = walz::review::to_watz(fixture.text()).unwrap();
    walz::compile(&walz::review::from_watz(&watz).unwrap().text).unwrap();
}

#[test]
fn unused_helpers_are_removed_and_aliases_keep_the_same_contract() {
    let source = r#"(module
      (@use "walz:core" (type $option) (func $i32.checked-add-u $sum))
      (func $main (export "main") (result (@inst $option i32)) (@effect pure) (call $sum 40 2)))"#;
    let compiled = walz::compile(source).unwrap();
    let module = compiled.ir.as_ref().unwrap();
    // Inspection retains all checked declarations; only emitted code is pruned.
    assert!(
        module
            .functions
            .iter()
            .any(|f| f.name.ends_with("i64.checked-mul-u"))
    );
    assert!(compiled.wat.contains("i32.checked-add-u"));
    assert!(!compiled.wat.contains("checked-mul"));
    assert!(!compiled.wat.contains("checked-align"));
    assert!(!compiled.wat.contains("i64."));
}

#[test]
fn boundary_cases_execute_in_wasmtime_with_and_without_typed_simplification() {
    let source = walz::source::load("tests/checked-integer.test.walz").unwrap();
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source.text(), walz::CompileOptions { optimize }).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("checked.wasm");
        std::fs::write(&path, &compiled.wasm).unwrap();
        let cases = &compiled.ir.as_ref().unwrap().tests;
        assert_eq!(cases.len(), 5);
        for case in cases {
            let output = std::process::Command::new("wasmtime")
                .args([
                    "run",
                    "-W",
                    "gc=y,function-references=y",
                    "--invoke",
                    &case.export,
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "1",
                "{}",
                case.name
            );
        }
    }
}
