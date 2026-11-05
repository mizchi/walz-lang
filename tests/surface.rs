use std::process::Command;

fn run(source: &str, expected: &str) {
    let compiled = wlt::compile(source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    let output = Command::new("wasmtime")
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
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
}

#[test]
fn prefixed_expressions_types_annotations_and_instantiations_are_checked() {
    run(
        r#"(module
      (%type.alias $Values (%type.params $T) (%list $T))
      (func $id (%type.params $T) (%where (shared $T) (nonunit $T))
        (param $x $T) (result $T) (%effects pure) (local.get $x))
      (func $main (export "main") (result i64) (%effects pure)
        (%let $xs (%inst $Values i64) (%list.new i64 (i64.const 42))
          (call (%inst $id i64) (%list.get i64 (local.get $xs) (i32.const 0))))))"#,
        "42",
    );
}

#[test]
fn unprefixed_extension_entries_are_not_accepted_in_strict_mode() {
    for source in [
        "(module (func $f (let $x (i32.const 0) (drop (local.get $x)))))",
        "(module (include \"missing.wlt\"))",
        "(module (import \"missing.wlt\" (func $f)))",
        "(module (export (func $f)) (func $f (nop)))",
        "(module (func $f (effects pure) (nop)))",
        "(module (func $f (param $s text) (nop)))",
        "(module (func $f (param $s (option i64)) (nop)))",
        "(module (func $f (type.params $T) (nop)))",
        "(module (tag $E) (func $f (try (do (nop)) (catch $E () (nop)))))",
        "(module (func $f (loop $again (bind $i i32 (i32.const 0)) (nop))))",
    ] {
        let error = wlt::compile(source).err().unwrap();
        assert_eq!(error.diagnostic().code, "E_WLT_PREFIX", "{source}\n{error}");
    }
}

#[test]
fn native_wat_keeps_its_own_loop_import_export_and_multi_result_semantics() {
    let source = r#"(module
      (func $pair (result i32 i64) i32.const 1 i64.const 2)
      (func $main (export "main") (result i32) (local $n i32)
        i32.const 42 local.set $n
        (loop $done (br_if $done (i32.const 0)))
        local.get $n))"#;
    assert_eq!(wlt::compile(source).unwrap().wat, source);
    run(source, "42");
}

#[test]
fn file_linking_is_explicit_and_never_interprets_a_wasm_type_import_as_a_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module (%export (func $answer)) (func $answer (result i32) (i32.const 42)))",
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        "(module (%import \"lib.wlt\" (func $answer)) (export \"main\" (func $answer)))",
    )
    .unwrap();
    let loaded = wlt::source::load(&main).unwrap();
    run(loaded.text(), "42");
    let proposal = "(module (import \"missing.wlt\" \"T\" (type $T (sub any))))";
    std::fs::write(&main, proposal).unwrap();
    assert_eq!(wlt::source::load(&main).unwrap().text(), proposal);
    std::fs::write(&main, "(module (import \"missing.wlt\" (func $answer)))").unwrap();
    let error = wlt::source::load(&main).err().unwrap();
    assert_eq!(error.diagnostic().code, "E_WLT_PREFIX");
}

#[test]
fn native_instructions_can_surround_a_prefixed_lexical_binding() {
    run(
        r#"(module (memory 1) (data (i32.const 0) "\2a\00\00\00")
      (func $main (export "main") (result i32) (local $unused i32)
        (%let $x i32 (i32.load (i32.const 0))
          (block (result i32)
            (local.set 0 (i32.const 1))
            (loop $done (br_if $done (i32.const 0)))
            (i32.add (local.get $x) (local.get 0))))))"#,
        "43",
    );
}

#[test]
fn native_macro_locals_respect_type_uses_and_quoted_identifier_identity() {
    run(
        r#"(module (type $Sig (func (param i32 i32) (result i32)))
      (func $inc (type $Sig)
        (%let $"x" (local.get 0x1)
          (i32.add (local.get $x) (local.get 0x0))))
      (memory 1) (data (i32.const 0) "\01\00\00\00")
      (func (export "main") (result i32) (call $inc (i32.const 41) (i32.const 1))))"#,
        "42",
    );
    for access in [
        "(local.set $\"x\" (i32.const 2))",
        "(local.set 0x0 (i32.const 2))",
    ] {
        let source = format!(
            "(module (memory 1) (func (export \"main\") (%let $x i32 (i32.load (i32.const 0)) {access})))"
        );
        assert!(wlt::compile(&source).is_err(), "{source}");
    }
}

#[test]
fn native_macro_binding_does_not_add_a_branch_label() {
    run(
        r#"(module (memory 1) (func (export "main") (result i32)
      (block (result i32)
        (%let $x i32 (i32.load (i32.const 0))
          (br 0 (i32.const 42)))
        (i32.const 1))))"#,
        "42",
    );
}

#[test]
fn native_fallback_never_weakens_recur_or_semantic_contracts() {
    for body in [
        "(%loop $l (bind $i i32 (i32.const 0)) (result i32) (i32.add (i32.const 1) (%recur $l (local.get $i))))",
        "(%loop $outer (result i32) (%loop $inner (result i32) (%recur $outer)))",
    ] {
        let source = format!("(module (memory 1) (func (result i32) {body}))");
        assert!(wlt::compile(&source).is_err(), "{source}");
    }
    for contract in ["(%effects pure)", "(%where (shared $T))"] {
        let source = format!(
            "(module (memory 1) (func {contract} (%let $x i32 (i32.load (i32.const 0)) (drop (local.get $x)))))"
        );
        assert!(wlt::compile(&source).is_err(), "{source}");
    }
}

#[test]
fn migration_preserves_native_wat_strings_comments_and_is_idempotent() {
    let legacy = r#"(module ;; let text import
      (func $f (export "let text") (result text)
        (let $x (text.lit "(let import)") (local.get $x))))"#;
    let migration = wlt::syntax::migrate(legacy).unwrap();
    assert!(migration.text().contains(";; let text import"));
    assert!(migration.text().contains("\"(let import)\""));
    assert!(migration.text().contains("(result %text)"));
    assert_eq!(
        wlt::syntax::migrate(migration.text()).unwrap().text(),
        migration.text()
    );
    wlt::check(migration.text()).unwrap();
    let native = r#"(module (func (result i32) (loop (result i32) (i32.const 42))))"#;
    assert_eq!(wlt::syntax::migrate(native).unwrap().text(), native);
}

#[test]
fn migration_cli_is_explicit_and_keeps_original_diagnostic_locations() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    let lib = dir.path().join("lib.wlt");
    let original =
        "(module\n (func $bad (result i64)\n  (let $x (i32.const 1) (local.get $missing))))";
    std::fs::write(&lib, original).unwrap();
    std::fs::write(&main, "(module (include \"lib.wlt\"))").unwrap();
    assert_eq!(
        wlt::source::load(&main).err().unwrap().diagnostic().code,
        "E_WLT_PREFIX"
    );
    let loaded = wlt::source::load_legacy(&main).unwrap();
    let error = loaded.diagnostic(wlt::check(loaded.text()).unwrap_err());
    let label = error.diagnostic().primary.as_ref().unwrap();
    assert_eq!(
        label.location.path.as_deref(),
        Some(lib.canonicalize().unwrap().as_path())
    );
    assert_eq!(label.location.line, 3);
    assert_eq!(
        label.location.column,
        original.lines().nth(2).unwrap().find("$missing").unwrap() + 1
    );
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_wlt"))
            .args(args)
            .arg(&lib)
            .output()
            .unwrap()
    };
    let preview = invoke(&["migrate"]);
    assert!(preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stdout).contains("(%let"));
    assert_eq!(std::fs::read_to_string(&lib).unwrap(), original);
    assert!(invoke(&["migrate", "--write"]).status.success());
    assert!(std::fs::read_to_string(&lib).unwrap().contains("(%let"));
}

#[test]
fn opaque_wat_annotations_are_never_wlt_expansion_or_file_directives() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    let source = r#"(module (@custom "opaque" (after code) "\ff")
      (@metadata (include "does-not-exist.wlt") (%unknown (let $x)))
      (memory 1) (func $main (export "main") (result i32)
        (%let $x i32 (i32.load (i32.const 0)) (i32.add (local.get $x) (i32.const 42)))))"#;
    std::fs::write(&path, source).unwrap();
    let loaded = wlt::source::load(&path).unwrap();
    let compiled = wlt::compile(loaded.text()).unwrap();
    assert!(compiled.ir.is_none());
    assert!(compiled.analysis.opaque);
    assert!(compiled.analysis.module_effects.host);
    assert_eq!(wlt::syntax::migrate(source).unwrap().text(), source);
    run(source, "42");
}

#[test]
fn native_state_loops_update_arguments_simultaneously_and_for_preserves_order() {
    run(
        r#"(module (memory 1) (func (export "main") (result i32)
      (%loop $swap (bind $a i32 (i32.const 1)) (bind $b i32 (i32.const 2)) (bind $n i32 (i32.const 0)) (result i32)
        (if (result i32) (i32.eqz (local.get $n))
          (then (%recur $swap (local.get $b) (local.get $a) (i32.const 1)))
          (else (i32.add (i32.mul (local.get $a) (i32.const 20)) (i32.mul (local.get $b) (i32.const 2))))))))"#,
        "42",
    );
    run(
        r#"(module (memory 1) (func (export "main") (result i32)
      (%for $count (bind $i i32 (i32.const 0))
        (while (i32.lt_u (local.get $i) (i32.const 6)))
        (step (i32.add (local.get $i) (i32.const 1)))
        (i32.store (i32.const 0) (i32.add (i32.load (i32.const 0)) (i32.const 7))))
      (i32.load (i32.const 0))))"#,
        "42",
    );
}

#[test]
fn native_fallback_cannot_bypass_opaque_type_privacy() {
    let source = r#"(module (memory 1)
      (%namespace $lib (%export (opaque $State))
        (type $State (struct (field i32))))
      (func $bad (result (ref $lib.State)) (struct.new $lib.State (i32.const 1))))"#;
    assert!(wlt::compile(source).is_err());
}
