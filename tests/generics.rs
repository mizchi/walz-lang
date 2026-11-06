// Compiler scope fixtures use private expanded forms; public files use %use paths.
use wlt::{CompileOptions, compile_with_options};

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compiled = compile_with_options(source, CompileOptions { optimize }).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("generics.wasm");
        std::fs::write(&file, compiled.wasm).unwrap();
        let output = std::process::Command::new("wasmtime")
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
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn functions_aliases_nominal_types_and_closures_share_explicit_type_application() {
    run(
        r#"(module
      (%type.alias $Values (%type.params $T) (%list $T))
      (type $Box (%type.params $T) (struct (field $T)))
      (%closure $Identity (%type.params $T) (param $x $T) (result $T) (local.get $x))
      (func $first (%type.params $T) (param $xs (%inst $Values $T)) (result $T)
        (%list.get $T (local.get $xs) (i32.const 0)))
      (func $apply (%type.params $T $F) (param $f (ref $F)) (param $x $T) (result $T)
        (%closure.call $F (local.get $f) (local.get $x)))
      (func $main (export "main") (result i64)
        (%let $box (struct.new (%inst $Box i64) (i64.const 42))
          (call (%inst $apply i64 (%inst $Identity i64)) (%closure.new (%inst $Identity i64))
            (call (%inst $first i64) (%list.new i64 (struct.get (%inst $Box i64) 0 (local.get $box))))))))"#,
        "42",
    );
}

#[test]
fn recursion_and_alias_arguments_reuse_one_concrete_instance() {
    let source = r#"(module
      (%type.alias $Count i64)
      (func $id (%type.params $T) (param $x $T) (result $T) (%effect pure) (local.get $x))
      (func $repeat (%type.params $T) (param $x $T) (param $n i32) (result $T)
        (if (result $T) (i32.eqz (local.get $n)) (then (local.get $x))
          (else (call (%inst $repeat $T) (local.get $x) (i32.sub (local.get $n) (i32.const 1))))))
      (func $main (export "main") (result i64)
        (call (%inst $repeat i64) (i64.add (call (%inst $id $Count) (i64.const 40)) (call (%inst $id i64) (i64.const 2))) (i32.const 3))))"#;
    run(source, "42");
    let module = wlt::check(source).unwrap();
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|f| f.name.starts_with("$id.mono"))
            .count(),
        1
    );
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|f| f.name.starts_with("$repeat.mono"))
            .count(),
        1
    );
}

#[test]
fn substitution_preserves_value_names_and_recursive_type_identity() {
    run(
        r#"(module
      (type $Node (%type.params $T) (struct (field $T) (field (ref null (%inst $Node $T)))))
      (func $id (%type.params $T) (param $T $T) (result $T) (local.get $T))
      (func $main (export "main") (result i32)
        (struct.get (%inst $Node i32) 0
          (struct.new (%inst $Node i32) (call (%inst $id i32) (i32.const 42)) (ref.null (%inst $Node i32))))))"#,
        "42",
    );
}

#[test]
fn instantiated_bodies_are_checked_before_dce_and_owned_arguments_keep_their_contracts() {
    run(
        r#"(module
      (func $id (%type.params $T) (param $x $T) (result $T) (%move $x))
      (func $main (export "main") (result i64)
        (%match (%box i64) (result i64) (call (%inst $id (%box i64)) (%box.new i64 (i64.const 42)))
          (case own ($x) (local.get $x)))))"#,
        "42",
    );
    for declaration in [
        "(func $bad (%type.params $T) (param $x $T) (result $T) (i32.const 1))",
        "(func $bad (%type.params $T) (param $x $T) (result $T) (local.get $x))",
    ] {
        let argument = if declaration.contains("i32.const") {
            "i64"
        } else {
            "(%box i64)"
        };
        let value = if argument == "i64" {
            "(i64.const 1)"
        } else {
            "(%box.new i64 (i64.const 1))"
        };
        let source = format!(
            "(module {declaration} (func $unused (result {argument}) (call (%inst $bad {argument}) {value})) (func $main (export \"main\") (nop)))"
        );
        assert!(
            wlt::compile(&source).is_err(),
            "unchecked instance: {source}"
        );
    }
}

#[test]
fn malformed_applications_and_infinite_specialization_report_errors() {
    let cases = [
        "(module (func $id (%type.params $T $T) (nop)))",
        "(module (func $id (%type.params) (nop)))",
        "(module (func $id (%type.params $T) (export \"id\") (nop)))",
        "(module (func $id (%type.params $T) (nop)) (func $main (call $id)))",
        "(module (func $id (%type.params $T) (nop)) (func $main (call (%inst $id))))",
        "(module (func $id (%type.params $T) (nop)) (func $main (call (%inst $id i32 i64))))",
        "(module (func $id (%type.params $T) (nop)) (func $main (call (%inst $id $Missing))))",
        "(module (func $id (%type.params $T) (nop)) (func $main (call (%inst $id (i32.const 0)))))",
        "(module (func $f (%type.params $T) (call (%inst $f (%list $T)))) (func $main (call (%inst $f i32))))",
        "(module (func $f (%type.params $T) (nop)) (func $f (nop)))",
        "(module (type $Box (%type.params $T) (struct (field $T))) (type $Box (struct)))",
        "(module (func $main (%list.from-array i32)))",
    ];
    for source in cases {
        assert!(wlt::compile(source).is_err(), "accepted {source}");
    }
}

#[test]
fn concrete_arguments_are_not_captured_by_other_type_parameters() {
    run(
        r#"(module
      (type $T (struct (field i32)))
      (func $id (%type.params $T $U) (param $x $T) (result $T) (local.get $x))
      (func $main (export "main") (result i32)
        (struct.get $T 0 (call (%inst $id (ref $T) i32) (struct.new $T (i32.const 42))))))"#,
        "42",
    );
}

#[test]
fn file_imports_share_instances_and_can_export_a_concrete_application() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("id.wlt"),
        r#"(module
      (%export (func $id))
      (func $id (%type.params $T) (param $x $T) (result $T) (local.get $x)))"#,
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        r#"(module
      (%use "./id.wlt" (func $id $a))
      (%use "./id.wlt" (func $id $b))
      (%type.alias $Count i64)
      (export "id" (func (%inst $a $Count)))
      (func $main (export "main") (result i64) (call (%inst $b i64) (i64.const 42))))"#,
    )
    .unwrap();
    let source = wlt::source::load(main).unwrap();
    let compiled = wlt::compile(source.text()).unwrap();
    let module = compiled.ir.as_ref().unwrap();
    let instances = module
        .functions
        .iter()
        .filter(|f| f.name.contains(".id.mono"))
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 1);
    assert!(instances[0].exports.contains(&"id".into()));
    run(source.text(), "42");
}

#[test]
fn generic_opaque_types_keep_their_template_namespace() {
    let declarations = r#"(%legacy.namespace $lib
      (%public (opaque $Box) (func $new) (func $get))
      (type $Box (%type.params $T) (struct (field $T)))
      (func $new (%type.params $T) (param $x $T) (result (ref (%inst $Box $T))) (struct.new (%inst $Box $T) (local.get $x)))
      (func $get (%type.params $T) (param $x (ref (%inst $Box $T))) (result $T) (struct.get (%inst $Box $T) 0 (local.get $x))))"#;
    run(
        &format!(
            "(module {declarations} (func $main (export \"main\") (result i32) (call (%inst $lib.get i32) (call (%inst $lib.new i32) (i32.const 42)))))"
        ),
        "42",
    );
    let source = format!(
        "(module {declarations} (func $bad (result i32) (struct.get (%inst $lib.Box i32) 0 (call (%inst $lib.new i32) (i32.const 42)))))"
    );
    let error = wlt::check(&source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_OPAQUE_REPRESENTATION");
    let source = format!(
        "(module {declarations}\n      (func $peek (%type.params $B) (param $x (ref $B)) (result i32) (struct.get $B 0 (local.get $x)))\n      (func $bad (result i32) (call (%inst $peek (%inst $lib.Box i32)) (call (%inst $lib.new i32) (i32.const 42)))))"
    );
    assert_eq!(
        wlt::check(&source).unwrap_err().diagnostic().code,
        "E_OPAQUE_REPRESENTATION"
    );
}

#[test]
fn generic_list_map_and_fold_are_concrete_and_preserve_callback_effects() {
    let source = wlt::source::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/generic-lists.wlt"),
    )
    .unwrap();
    run(source.text(), "42");
    let compiled = wlt::compile(source.text()).unwrap();
    assert!(!compiled.wat.contains("call_indirect"));
    assert!(!compiled.wat.contains("call_ref"));
    assert!(
        compiled
            .analysis
            .functions
            .iter()
            .any(|f| f.name.contains(".map.mono") && f.effects.writes)
    );
    let module = compiled.ir.as_ref().unwrap();
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|f| f.name.contains(".map.mono"))
            .count(),
        3
    );
}

#[test]
fn expansion_limits_and_invalid_unused_type_arguments_are_diagnostics() {
    for argument in ["(ref i32)", "(%option i32 i64)", "$Box", "(ref null)"] {
        let source = format!(
            "(module (type $Box (%type.params $T) (struct (field $T))) (func $f (%type.params $T) (nop)) (func $main (call (%inst $f {argument}))))"
        );
        assert!(wlt::compile(&source).is_err(), "accepted {source}");
    }
    let mut source = String::from("(module");
    for i in 0..70 {
        source.push_str(&format!(
            "(func $f{i} (%type.params $T) (call (%inst $f{} $T)))",
            i + 1
        ));
    }
    source.push_str("(func $f70 (%type.params $T) (nop)) (func $main (call (%inst $f0 i32))))");
    let error = wlt::compile(&source).err().unwrap();
    assert!(error.to_string().contains("limit"));
    let source = "(module (func $id (%type.params $T) (param $x $T) (result $T) (i32.const 0)) (func $main (result i64) (call (%inst $id i64) (i64.const 1))))";
    let error = wlt::check(source).unwrap_err();
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|l| l.message.contains("instantiation of $id<i64>"))
    );
}

#[test]
fn transparent_generic_aliases_share_keys_and_generated_names_avoid_source_names() {
    let source = r#"(module
      (%type.alias $Values (%type.params $T) (%list $T))
      (%type.alias $Numbers (%inst $Values i64))
      (func $id (%type.params $T) (param $x $T) (result $T) (local.get $x))
      (func $id.mono0 (result i32) (i32.const 0))
      (func $a (result $Numbers) (call (%inst $id $Numbers) (%list.new i64)))
      (func $b (result (%list i64)) (call (%inst $id (%list i64)) (%list.new i64)))
      (func $main (export "main") (result i32) (i32.add (call $id.mono0) (%list.len i64 (call $a)))))"#;
    run(source, "0");
    let module = wlt::check(source).unwrap();
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|f| f.name.starts_with("$id.mono"))
            .count(),
        2
    );
}

#[test]
fn callback_effects_are_checked_after_specialization() {
    let source = r#"(module
      (type $Cell (struct (field (mut i64))))
      (%closure $Set (%capture $cell (ref $Cell)) (param $x i64) (result i64)
        (block (result i64) (struct.set $Cell 0 (local.get $cell) (local.get $x)) (local.get $x)))
      (func $apply (%type.params $F) (param $f (ref $F)) (result i64) (%effect pure)
        (%closure.call $F (local.get $f) (i64.const 42)))
      (func $main (export "main") (result i64)
        (call (%inst $apply $Set) (%closure.new $Set (struct.new $Cell (i64.const 0))))))"#;
    let error = wlt::compile(source).err().unwrap();
    assert!(error.to_string().contains("effect"));
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|l| l.message.contains("$apply<$Set>"))
    );
}

#[test]
fn generic_variant_matches_and_constructor_type_arguments_are_expanded() {
    run(
        r#"(module
      (type $Choice (%type.params $T) (%variant (empty) (value $T)))
      (%type.alias $Item (%type.params $T) $T)
      (func $main (export "main") (result i64)
        (%match (%inst $Choice i64) (result i64) (%variant.new (%inst $Choice i64) value (i64.const 42))
          (case empty () (i64.const 0))
          (case value ($x)
            (%match (%option i64) (result i64) (%option.some (%inst $Item i64) (local.get $x))
              (case none () (i64.const 0))
              (case some ($y) (local.get $y)))))))"#,
        "42",
    );
}
