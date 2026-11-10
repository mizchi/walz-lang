use std::process::Command;

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let out = Command::new("wasmtime")
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
fn hundreds_of_independent_functions_expand_with_the_existing_limits() {
    for count in [256, 512] {
        let mut source = String::from(
            "(module (@macro $double (rule (?x:expr) (infer (?T ?x)) (when (Add ?x)) (@let $value (type ?T) ?x (+ $value $value))))",
        );
        for n in 0..count {
            source.push_str(&format!(
                "(func $f{n} (param $x i64) (result i64) (@effect pure) (@$double $x))"
            ));
        }
        source.push(')');
        let report = walz::macros::report(&source, &Default::default()).unwrap();
        assert_eq!(report.expansions.len(), count);
        assert_eq!(report.checking.module_passes, 1);
        assert_eq!(report.checking.function_checks, count);
    }
}

#[test]
fn local_expansion_and_a_later_generic_restart_preserve_types_and_origins() {
    run(
        r#"(module
      (@type.alias $Wide i64)
      (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
      (@macro $local (rule (?x:expr) (@let $saved (type $Wide) ?x (+ $saved 1))))
      (@macro $generic (rule (?x:expr) (infer (?T ?x)) (call (@inst $id ?T) ?x)))
      (func $main (export "main") (result i64) (@effect pure)
        (+ (@$local (i64.const 20)) (@$generic (@$local (i64.const 20))))))"#,
        "42",
    );
}

#[test]
fn binding_contracts_can_be_delegated_across_multiple_macro_layers() {
    run(
        r#"(module
      (@macro $inner (rule (?name:ident ?value:expr ?body:expr)
        (bind ?name ?body) (@let ?name ?value ?body)))
      (@macro $middle (rule (?name:ident ?value:expr ?body:expr)
        (bind ?name ?body) (@$inner ?name ?value ?body)))
      (@macro $outer (rule (?name:ident ?value:expr ?body:expr)
        (bind ?name ?body) (@$middle ?name ?value ?body)))
      (func $main (export "main") (result i32) (@effect pure)
        (@let $answer (type i32) 40
          (@$outer $answer (+ $answer 1) (+ $answer 1)))))"#,
        "42",
    );
}

#[test]
fn delegation_does_not_grant_binding_to_initializers_or_undeclared_bodies() {
    for clauses in ["(bind ?name ?body)", ""] {
        let source = format!(
            "(module (@macro $inner (rule (?n:ident ?value:expr ?body:expr) (bind ?n ?body) (@let ?n ?value ?body))) (@macro $outer (rule (?name:ident ?body:expr) {clauses} (@$inner ?name ?body ?body))) (func $f (result i32) (@$outer $n $n)))"
        );
        let error = walz::check(&source).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_MACRO_BINDING", "{error}");
    }
}

#[test]
fn repeated_binders_expand_before_let_lowering_and_preserve_caller_hygiene() {
    run(
        r#"(module
      (@macro $bindings
        (rule (((?name:ident ?value:expr) ...) ?body:expr)
          (bind ?name ?body) (@let ((?name ?value) ...) ?body)))
      (func $main (export "main") (result i32) (@effect pure)
        (@let $a (type i32) 20
          (@$bindings (($a (+ $a 1)) ($b (i32.const 21)))
            (+ $a $b)))))"#,
        "42",
    );
    run(
        r#"(module
      (@macro $bindings
        (rule (((?name:ident ?value:expr) ...) ?body:expr)
          (bind ?name ?body) (@let ((?name ?value) ...) ?body)))
      (func $main (export "main") (result i32) (@effect pure)
        (@$bindings () (i32.const 42))))"#,
        "42",
    );
}

#[test]
fn repeated_binders_are_unique_and_initializers_do_not_capture_new_names() {
    let header = "(module (@macro $bindings (rule (((?name:ident ?value:expr) ...) ?body:expr) (bind ?name ?body) (@let ((?name ?value) ...) ?body)))";
    for (body, code) in [
        (
            "(@$bindings (($a (i32.const 1)) ($a (i32.const 2))) $a)",
            "E_MACRO_BINDING",
        ),
        (
            "(@$bindings (($a (i32.const 1)) ($b $a)) $b)",
            "E_UNKNOWN_LOCAL",
        ),
    ] {
        let source = format!("{header} (func $f (result i32) {body}))");
        let error = walz::check(&source).unwrap_err();
        assert_eq!(error.diagnostic().code, code, "{error}");
    }
}

#[test]
fn repeated_binding_interfaces_can_be_delegated() {
    run(
        r#"(module
      (@macro $inner (rule (((?n:ident ?v:expr) ...) ?body:expr)
        (bind ?n ?body) (@let ((?n (type i64) ?v) ...) ?body)))
      (@macro $outer (rule (((?n:ident ?v:expr) ...) ?body:expr)
        (bind ?n ?body) (@$inner ((?n ?v) ...) ?body)))
      (func $main (export "main") (result i64) (@effect pure)
        (@$outer (($x (i64.const 20)) ($y (i64.const 22))) (+ $x $y))))"#,
        "42",
    );
}

#[test]
fn inconsistent_delegated_interfaces_are_rejected_even_without_an_outer_bind() {
    let source = r#"(module
      (@macro $inner
        (rule (?n:ident ?v:expr ?body:expr) (bind ?n ?body) (@let ?n ?v ?body))
        (rule (?n:ident ?v:expr ?body:expr) ?body))
      (@macro $outer (rule (?n:ident ?v:expr ?body:expr) (@$inner ?n ?v ?body)))
      (func $f (result i32) (@$outer $x (i32.const 1) $x)))"#;
    assert_eq!(
        walz::check(source).unwrap_err().diagnostic().code,
        "E_MACRO_BINDING"
    );
}

#[test]
fn macro_output_keeps_type_and_function_namespaces_distinct_and_invalid_types_diagnostic() {
    run(
        r#"(module
      (type $Same (@type.params $T) (struct (field $T)))
      (func $Same (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
      (@macro $m (rule (?x:expr) (call (@inst $Same i64) ?x)))
      (func $main (export "main") (result i64) (@effect allocates)
        (block (result i64) (drop (struct.new (@inst $Same i64) 0)) (@$m (i64.const 42)))))"#,
        "42",
    );
    let source = "(module (func $id (@type.params $T) (param $x $T) (result $T) $x) (@macro $m (rule () (call (@inst $id \"bad\") (i32.const 1)))) (func $f (result i32) (@$m)))";
    assert!(
        std::panic::catch_unwind(|| walz::check(source))
            .unwrap()
            .is_err()
    );
}

#[test]
fn first_repeated_initializer_is_normalized_and_resolves_imported_helpers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("helper.walz"),"(module (func $helper (param $n i64) (result i64) (@effect pure) $n) (@export (func $helper)))").unwrap();
    let path = dir.path().join("main.walz");
    let source = r#"(module
      (@use "./helper.walz" (func $helper $id))
      (@macro $bindings (rule (((?n:ident ?value:expr) ...) ?body:expr)
        (bind ?n ?body) (@let ((?n ?value) ...) ?body)))
      (func $main (export "main") (result i64) (@effect allocates)
        (@$bindings (($owner (@box.new i64 (call $id (i64.const 42)))))
          (@match (@box i64) (result i64) (@move $owner) (case own ($value) $value)))))"#;
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load(&path).unwrap();
    run(loaded.text(), "42");
}

#[test]
fn cached_generic_aliases_are_expanded_after_resolving_macro_type_applications() {
    run(
        r#"(module
      (@use "walz:core" (type $option))
      (@macro $unwrap (rule (?x:expr) (infer (?T (option.element ?x)))
        (@match (@inst $option ?T) (result ?T) ?x (case none () (unreachable)) (case some ($value) $value))))
      (func $get (param $x (@inst $option i64)) (result i64) (@effect pure) (@$unwrap $x))
      (func $main (export "main") (result i64) (@effect pure) (call $get (@option.some i64 42))))"#,
        "42",
    );
}

#[test]
fn root_delegation_chains_are_replayed_before_a_generic_environment_restart() {
    let source = r#"(module
      (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
      (@macro $generic (rule (?x:expr) (call (@inst $id i64) ?x)))
      (@macro $bindings (rule (((?n:ident ?v:expr) ...) ?body:expr)
        (bind ?n ?body) (@let ((?n (type i64) ?v) ...) ?body)))
      (@macro $delegated (rule (((?n:ident ?v:expr) ...) ?body:expr)
        (bind ?n ?body) (@$bindings ((?n ?v) ...) ?body)))
      (func $main (export "main") (result i64) (@effect pure)
        (@$delegated (($x (@$generic (i64.const 42)))) $x)))"#;
    run(source, "42");
    assert_eq!(
        walz::macros::report(source, &Default::default())
            .unwrap()
            .expansions
            .len(),
        3
    );
}

#[test]
fn repeated_type_fragments_resolve_in_the_callers_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("types.walz"),
        "(module (@type.alias $Count i32) (@export (type $Count)))",
    )
    .unwrap();
    let source = r#"(module
      (@use "./types.walz" (type $Count))
      (@macro $typed (rule (((?n:ident ?T:type ?v:expr) ...) ?body:expr)
        (bind ?n ?body) (@let ((?n (type ?T) ?v) ...) ?body)))
      (func $main (export "main") (result i64) (@effect pure)
        (@$typed (($x $Count 20) ($y i64 22))
          (+ (i64.extend_i32_s $x) $y))))"#;
    let path = dir.path().join("main.walz");
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load(&path).unwrap();
    run(loaded.text(), "42");
}
