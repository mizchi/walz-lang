use std::process::Command;

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("inferred.wasm");
        std::fs::write(&file, compiled.wasm).unwrap();
        let out = Command::new("wasmtime")
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
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), expected);
    }
    walz::component::compile(source).unwrap();
    walz::component::core_js::compile(source).unwrap();
}

#[test]
fn argument_and_expected_types_resolve_nested_calls_without_a_default_width() {
    run(
        r#"(module
      (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
      (func $pick (@type.params $T) (param $a $T) (param $b $T) (result $T) (@effect pure) $b)
      (func $main (export "main") (result i64) (@effect pure)
        (@let $value (call $pick 1 (i64.const 41))
          (call $id (+ (call $id $value) 1)))))"#,
        "42",
    );
    run(
        r#"(module
      (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
      (func $main (export "main") (result i32) (@effect pure) (call $id 42)))"#,
        "42",
    );
    for source in [
        "(module (func $id (@type.params $T) (param $x $T) (result $T) $x) (func $f (@let $x (call $id 1) (drop $x))))",
        "(module (func $phantom (@type.params $T) (result i32) 42) (func $f (result i32) (call $phantom)))",
        "(module (func $same (@type.params $T) (param $x $T) (param $y $T) (result $T) $x) (func $f (@let $x (call $same (i32.const 1) (i64.const 2)) (drop $x))))",
    ] {
        let error = walz::check(source).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_GENERIC_INFERENCE", "{error}");
        assert!(error.to_string().contains("@inst"), "{error}");
    }
}

#[test]
fn result_context_resolves_a_generic_producer_and_preserves_explicit_applications() {
    run(
        r#"(module
      (@use "walz:core" (type $option))
      (func $empty (@type.params $T) (result (@inst $option $T)) (@effect pure) (@option.none $T))
      (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
      (func $main (export "main") (result i32) (@effect pure)
        (@let $value (type (@inst $option i64)) (call $empty)
          (@match $value (case none () (call (@inst $id i32) 42)) (case some ($n) (i32.wrap_i64 $n))))))"#,
        "42",
    );
}

#[test]
fn constructed_aliases_and_nominal_applications_unify_structurally() {
    run(
        r#"(module
      (@type.alias $Values (@type.params $T) (@list $T))
      (type $Box (@type.params $T) (struct (field $value $T)))
      (func $first (@type.params $T) (param $xs (@inst $Values $T)) (result $T) (@effect pure)
        (@list.get $T $xs 0))
      (func $unbox (@type.params $T) (param $box (ref (@inst $Box $T))) (result $T) (@effect pure)
        (struct.get (@inst $Box $T) $value $box))
      (func $main (export "main") (result i64) (@effect allocates)
        (+ (call $unbox (struct.new (@inst $Box i64) (i64.const 40)))
           (call $first (@list.new i64 2)))))"#,
        "42",
    );
}

#[test]
fn closure_argument_signature_resolves_map_output_without_context() {
    run(
        r#"(module
      (@use "walz:core" (type $list) (func $list.map))
      (@closure $Convert (param $x i32) (result i64) (@effect pure) (i64.extend_i32_s $x))
      (func $main (export "main") (result i64) (@effect reads writes allocates)
        (@let $mapped (call $list.map (@list.new i32 20 22) (@closure.new $Convert))
          (+ (@list.get i64 $mapped 0) (@list.get i64 $mapped 1)))))"#,
        "42",
    );
}

#[test]
fn inference_keeps_owned_moves_bounds_and_effect_obligations() {
    run(
        r#"(module
      (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) (@move $x))
      (func $main (export "main") (result i32) (@effect allocates)
        (@let $box (@box.new i32 42)
          (@match (@box i32) (result i32) (call $id (@move $box)) (case own ($x) $x)))))"#,
        "42",
    );
    for (source, expected) in [
        (
            "(module (func $id (@type.params $T) (@where (shared $T)) (param $x $T) (result $T) $x) (func $f (@let $x (call $id (@box.new i32 1)) (@box.close (@move $x)))))",
            "E_GENERIC_BOUND",
        ),
        (
            "(module (import \"host\" \"f\" (func $f (result i32))) (func $call (@type.params $T) (param $x $T) (result $T) (block (result $T) (drop (call $f)) $x)) (func $main (result i32) (@effect pure) (call $call (i32.const 1))))",
            "E_EFFECT",
        ),
    ] {
        let error = walz::check(source).unwrap_err();
        assert!(error.diagnostic().code.starts_with(expected), "{error}");
    }
}

#[test]
fn recursive_calls_reuse_the_instance_and_changing_types_is_rejected() {
    run(
        r#"(module
      (func $sum (@type.params $T) (@where (Add $T)) (param $x $T) (param $n i32)
        (result $T) (@effect pure)
        (if (result $T) (= $n 0) (then $x) (else (call $sum (+ $x $x) (- $n 1)))))
      (func $main (export "main") (result i64) (@effect pure) (call $sum (i64.const 21) 1)))"#,
        "42",
    );
    let error=walz::check("(module (func $grow (@type.params $T) (param $x $T) (result i32) (call $grow (@list.new $T $x))) (func $f (result i32) (call $grow (i32.const 1))))").unwrap_err();
    assert!(
        error.to_string().contains("polymorphic recursion"),
        "{error}"
    );
}

#[test]
fn many_call_sites_share_one_inferred_specialization() {
    let calls = (0..256)
        .map(|i| format!("(drop (call $id (i64.const {i})))"))
        .collect::<String>();
    let source = format!(
        "(module (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x) (func $f (block {calls})))"
    );
    let module = walz::check(&source).unwrap();
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|f| f.name.starts_with("$id.mono"))
            .count(),
        1
    );
}

#[test]
fn imported_aliases_reexports_and_type_inspection_keep_source_locations() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("id.walz"),"(module (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x) (@export (func $id)))").unwrap();
    std::fs::write(
        directory.path().join("facade.walz"),
        "(module (@use \"./id.walz\" (func $id $forward)) (@export (func $forward)))",
    )
    .unwrap();
    let path = directory.path().join("main.walz");
    let source = "(module (@use \"./facade.walz\" (func $forward $answer)) (func $main (export \"main\") (result i32) (@effect pure) (call $answer (i32.const 42))))";
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load(&path).unwrap();
    run(loaded.text(), "42");
    let column = source.rfind("i32.const").unwrap() + 1;
    let report = walz::inspect::type_at(&loaded, None, 1, column, &Default::default()).unwrap();
    assert!(report.matches.iter().any(|m| m.ty == "i32"), "{report:?}");
}

#[test]
fn cyclic_aliases_cannot_turn_type_inference_into_unbounded_expansion() {
    let source = "(module (@type.alias $Loop (@type.params $T) (@inst $Loop $T)) (func $f (@type.params $T) (param $x (@inst $Loop $T)) (result i32) 0) (func $g (result i32) (call $f (i32.const 1))))";
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_GENERIC_INFERENCE", "{error}");
    assert!(error.to_string().contains("limit"), "{error}");
}

#[test]
fn malformed_generic_type_syntax_is_diagnosed_without_panicking() {
    for ty in ["\"bad\"", "(@list \"bad\")", "(ref \"bad\")"] {
        let source = format!(
            "(module (func $f (@type.params $T) (param $x {ty}) (result $T) (unreachable)) (func $g (result i32) (call $f (i32.const 0))))"
        );
        let checked = std::panic::catch_unwind(|| walz::check(&source));
        assert!(checked.is_ok(), "panicked for {ty}");
        assert!(checked.unwrap().is_err(), "accepted {ty}");
    }
}

#[test]
fn expected_results_supply_full_nominal_constructor_hints() {
    run(
        r#"(module
      (@use "walz:core" (type $option))
      (type $Box (@type.params $T) (struct (field $value $T)))
      (func $take (@type.params $T) (param $x (@inst $option (ref (@inst $Box $T))))
        (result $T) (@effect pure)
        (@match $x (case none () (unreachable))
          (case some ($box) (struct.get (@inst $Box $T) $value $box))))
      (func $main (export "main") (result i64) (@effect allocates)
        (call $take (@option.some (struct.new (@inst $Box i64) 42)))))"#,
        "42",
    );
}
