use std::process::Command;

fn run(source: &str, expected: &str) {
    let source = source
        .replace(
            "(export \"main\") (result i32)",
            "(export \"main\") (result i32) (@effect reads writes allocates)",
        )
        .replace(
            "(export \"main\") (result i64)",
            "(export \"main\") (result i64) (@effect reads writes allocates)",
        );
    let source = source.as_str();
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("macros.wasm");
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
fn templates_are_hygienic_and_inputs_are_evaluated_once_when_bound() {
    run(
        r#"(module
      (type $Counter (struct (field $n (mut i32))))
      (@macro $twice
        (rule (?value:expr)
          (@let $tmp ?value (+ $tmp $tmp))))
      (func $next (param $c (ref $Counter)) (result i32)
        (block (result i32)
          (struct.set $Counter $n $c (+ (struct.get $Counter $n $c) 1))
          (struct.get $Counter $n $c)))
      (func $main (export "main") (result i32)
        (@let (($tmp (type i32) 20) ($c (struct.new $Counter 0)))
          (+ (@$twice (+ $tmp (call $next $c)))
             (- (struct.get $Counter $n $c) 1)))))"#,
        "42",
    );
}

#[test]
fn inferred_types_drive_rules_type_templates_and_generic_specializations() {
    run(
        r#"(module
      (func $id (@type.params $T) (param $x $T) (result $T) $x)
      (@macro $identity
        (rule (?x:expr)
          (infer (?T ?x))
          (call (@inst $id ?T) ?x)))
      (@macro $width
        (rule (?x:expr) (when (type ?x i32)) (i32.const 32))
        (rule (?x:expr) (when (type ?x i64)) (i32.const 64)))
      (@macro $double
        (rule (?x:expr) (when (Add ?x)) (@let $v ?x (+ $v $v))))
      (func $use (@type.params $T) (param $x $T) (result $T)
        (@$identity $x))
      (func $main (export "main") (result i32)
        (+ (call (@inst $use i32) (@$double (i32.const 5)))
           (- (@$width (call (@inst $use i64) (i64.const 9))) 32))))"#,
        "42",
    );
}

#[test]
fn expected_types_are_explicit_and_never_guess_numeric_widths() {
    run(
        r#"(module
      (@macro $zero (rule () (infer (?T expected)) (@let $x (type ?T) 0 $x)))
      (func $main (export "main") (result i64) (+ (@$zero) 42)))"#,
        "42",
    );
    let error = walz::check(
        "(module (@macro $id (rule (?x:expr) (infer (?T ?x)) ?x)) (func $f (result i32) (@$id 1)))",
    )
    .unwrap_err();
    assert_eq!(error.diagnostic().code, "E_LITERAL_TYPE", "{error}");
}

#[test]
fn explicit_binders_scope_only_the_declared_body_and_respect_shadowing() {
    run(
        r#"(module
      (@macro $with
        (rule (?name:ident ?value:expr ?body:expr)
          (bind ?name ?body)
          (@let ?name ?value ?body)))
      (func $main (export "main") (result i32)
        (@let $x (type i32) 40
          (@$with $x (+ $x 1)
            (+ $x (@let $x (type i32) 1 $x))))))"#,
        "42",
    );
    for source in [
        "(module (@macro $m (rule (?n:ident ?v:expr ?b:expr) (@let ?n ?v ?b))) (func $f (result i32) (@$m $x 1 $x)))",
        "(module (@macro $m (rule (?n:ident ?b:expr) (bind ?n ?b) ?b)) (func $f (result i32) (@$m $x $x)))",
        "(module (@macro $m (rule () $hidden)) (func $f (param $hidden i32) (result i32) (@$m)))",
    ] {
        let error = walz::check(source).unwrap_err();
        assert!(
            matches!(
                error.diagnostic().code,
                "E_MACRO_BINDING" | "E_MACRO_HYGIENE"
            ),
            "{error}"
        );
    }
}

#[test]
fn trailing_repetition_and_nested_calls_compose_without_reader_changes() {
    run(
        r#"(module
      (@macro $sum
        (rule (?x:expr) ?x)
        (rule (?x:expr ?rest:expr ...) (+ ?x (@$sum ?rest ...))))
      (func $main (export "main") (result i32)
        (@$sum (i32.const 10) (i32.const 12) (i32.const 20))))"#,
        "42",
    );
    let wat = "(module (func (export \"main\") (result i32) i32.const 42))";
    assert_eq!(walz::expand(wat).unwrap(), wat);
}

#[test]
fn file_macros_keep_definition_site_helpers_private_and_support_reexports() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("syntax.walz"),
        r#"(module
      (func $helper (param $x i32) (result i32) (+ $x 2))
      (@macro $answer (rule (?x:expr) (call $helper ?x)))
      (@macro $secret (rule () (i32.const 0)))
      (@export (macro $answer)))"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("facade.walz"),
        "(module (@use \"./syntax.walz\" (macro $answer $forward)) (@export (macro $forward)))",
    )
    .unwrap();
    let main = dir.path().join("main.walz");
    std::fs::write(
        &main,
        r#"(module
      (@use "./facade.walz" (macro $forward $answer))
      (func $helper (param $x i32) (result i32) (i32.const 999))
      (func $main (export "main") (result i32) (@$answer (i32.const 40))))"#,
    )
    .unwrap();
    let loaded = walz::source::load(&main).unwrap();
    run(loaded.text(), "42");
    std::fs::write(&main, "(module (@use \"./syntax.walz\" (macro $secret)))").unwrap();
    let loaded = walz::source::load(&main).unwrap();
    assert!(
        walz::check(loaded.text())
            .unwrap_err()
            .to_string()
            .contains("private")
    );
}

#[test]
fn expansion_cannot_bypass_effects_ownership_or_checked_core_validation() {
    for (source, message) in [
        (
            "(module (import \"host\" \"f\" (func $f (result i32))) (@macro $m (rule () (call $f))) (func $main (result i32) (@effect pure) (@$m)))",
            "effect",
        ),
        (
            "(module (@macro $m (rule (?x:expr) (block ?x ?x))) (func $main (@let $b (@box.new i32 1) (@$m (@box.close (@move $b))))))",
            "already moved",
        ),
        (
            "(module (@macro $m (rule () (memory 1))) (func $f (@$m)))",
            "expression",
        ),
    ] {
        let error = walz::compile(source)
            .err()
            .expect("unsafe expansion must fail");
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn malformed_patterns_unknown_macros_and_expansion_limits_are_diagnostics() {
    for source in [
        "(module (@macro $m (rule (?x:wat) ?x)))",
        "(module (@macro $m (rule (?x:expr ?x:expr) ?x)))",
        "(module (@macro $m (rule () ?missing)))",
        "(module (func $f (@$missing)))",
        "(module (@macro $m (rule () (@$m))) (func $f (@$m)))",
    ] {
        let error = walz::compile(source).err().unwrap();
        assert!(error.diagnostic().code.starts_with("E_MACRO"), "{error}");
        assert!(error.diagnostic().primary.is_some());
    }
}

#[test]
fn argument_errors_keep_their_span_and_definition_errors_have_expansion_context() {
    let source = "(module\n (@macro $m (rule (?x:expr) (i32.add ?x (i64.const 1))))\n (func $f (result i32) (@$m (i32.const 2))))";
    let error = walz::check(source).unwrap_err();
    assert_eq!(
        error.diagnostic().primary.as_ref().unwrap().location.line,
        3,
        "{error}"
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|l| l.location.line == 2),
        "{error}"
    );
    let source = "(module (@macro $m (rule (?x:expr) (i32.add ?x 1))) (func $f (result i32) (@$m (local.get $missing))))";
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_UNKNOWN_LOCAL");
    assert_eq!(
        &source[error.diagnostic().primary.as_ref().unwrap().span.start
            ..error.diagnostic().primary.as_ref().unwrap().span.end],
        "$missing"
    );
}

#[test]
fn reports_retain_inferred_types_duplicate_origins_and_nested_expansion_history() {
    let source = "(module (@macro $id (rule (?x:expr) (infer (?T ?x)) (@let $v (type ?T) ?x $v))) (@macro $twice (rule (?x:expr) (+ (@$id ?x) (@$id ?x)))) (func $f (param $n i32) (result i32) (@$twice $n)))";
    let report = walz::macros::report(source, &Default::default()).unwrap();
    assert_eq!(report.expansions.len(), 3);
    assert_eq!(report.expansions[1].parent, Some(0));
    assert_eq!(report.expansions[2].inferred["?T"], "i32");
    let original = source.rfind("$n").unwrap();
    assert!(
        report.expansions[0]
            .nodes
            .iter()
            .filter(|n| n.source.start == original)
            .count()
            >= 2
    );
}

#[test]
fn nominal_and_constructed_inferred_types_and_alias_guards_remain_concrete() {
    run(
        r#"(module
      (@type.alias $Count i32)
      (type $Box (struct (field $value i32)))
      (func $id (@type.params $T) (param $v $T) (result $T) $v)
      (@macro $id (rule (?v:expr) (infer (?T ?v)) (call (@inst $id ?T) ?v)))
      (@macro $count (rule (?v:expr) (when (type ?v $Count)) ?v))
      (func $main (export "main") (result i32)
        (+ (@$count (struct.get $Box $value (@$id (struct.new $Box 40))))
           (@list.get i32 (@$id (@list.new i32 2 3)) 0))))"#,
        "42",
    );
}

#[test]
fn explicit_type_fragments_resolve_in_the_callers_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("macros.walz"),"(module (@macro $typed (rule (?T:type ?v:expr) (@let $x (type ?T) ?v $x))) (@export (macro $typed)))").unwrap();
    std::fs::write(dir.path().join("lib.walz"),"(module (@use \"./macros.walz\" (macro $typed)) (@type.alias $Count i32) (func $answer (result i32) (@$typed $Count 42)) (@export (func $answer)))").unwrap();
    let file = dir.path().join("main.walz");
    std::fs::write(&file,"(module (@use \"./lib.walz\" (func $answer)) (func $main (export \"main\") (result i32) (call $answer)))").unwrap();
    let loaded = walz::source::load(&file).unwrap();
    run(loaded.text(), "42");
}

#[test]
fn intentional_scope_is_preserved_through_nested_captured_macro_calls() {
    run(
        r#"(module
      (@macro $id (rule (?v:expr) ?v))
      (@macro $with (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@let ?n ?v ?b)))
      (func $main (export "main") (result i32)
        (@$with $x (i32.const 42) (@$id $x))))"#,
        "42",
    );
}

#[test]
fn templates_reuse_checked_memory_and_sequential_binding_forms() {
    run(
        r#"(module
      (memory $data 1)
      (@macro $load (rule (?p:expr) (i32.load $data ?p)))
      (func $main (export "main") (result i32)
        (block (result i32)
          (i32.store $data (i32.const 0) (i32.const 42))
          (@$load (i32.const 0)))))"#,
        "42",
    );
    run(
        r#"(module
      (@macro $chain (rule (?value:expr)
        (@result.do ($saved ?value) (@result.ok i32 i32 (+ $saved 1)))))
      (func $main (export "main") (result i32)
        (@match (@result i32 i32) (result i32) (@$chain (@result.ok i32 i32 41))
          (@case ok ($value) $value) (@case err ($error) $error))))"#,
        "42",
    );
}

#[test]
fn macro_heads_are_syntax_signatures_instead_of_generated_operand_types() {
    let source = "(module (@macro $narrow (rule (?x:expr) (@let $saved ?x (i32.wrap_i64 $saved)))) (func $f (param $x i64) (result i32) (@$narrow $x)))";
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.walz");
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load(&path).unwrap();
    let column = source.find("@$narrow").unwrap() + 1;
    let result = walz::inspect::type_at(&loaded, None, 1, column, &Default::default()).unwrap();
    assert!(result.matches.is_empty(), "{result:?}");
}
