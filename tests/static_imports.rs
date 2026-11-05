use std::path::Path;

fn compile_file(path: &Path) -> Result<wlt::Compilation, wlt::Error> {
    let source = wlt::source::load(path)?;
    wlt::compile(source.text()).map_err(|e| source.diagnostic(e))
}

#[test]
fn named_file_imports_are_private_cached_and_selectively_linked() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("math.wlt"),
        r#"(module
      (export (func $answer) (func $other))
      (func $helper (result i32) (i32.const 42))
      (func $answer (result i32) (call $helper))
      (func $other (result i32) (i32.const 9)))"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("facade.wlt"),
        r#"(module
      (import "math.wlt" (func $answer $forward))
      (export (func $forward)))"#,
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        r#"(module
      (import "math.wlt" (func $answer $direct))
      (import "./facade.wlt" (func $forward))
      (func $main (export "main") (result i32)
        (i32.add (call $direct) (call $forward))))"#,
    )
    .unwrap();
    let compiled = compile_file(&main).unwrap();
    assert!(!compiled.wat.contains(".other"));
    let functions = &compiled.ir.as_ref().unwrap().functions;
    assert_eq!(
        functions
            .iter()
            .filter(|f| f.name.ends_with(".answer"))
            .count(),
        1
    );
    let wasm = dir.path().join("module.wasm");
    std::fs::write(&wasm, compiled.wasm).unwrap();
    let output = std::process::Command::new("wasmtime")
        .args(["run", "--invoke", "main"])
        .arg(wasm)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "84");
    for reference in ["$helper", "$answer", "$other"] {
        std::fs::write(&main, format!("(module (import \"math.wlt\" (func $answer $chosen)) (func $main (export \"main\") (result i32) (call {reference})))")).unwrap();
        assert!(compile_file(&main).is_err(), "{reference}");
    }
}

#[test]
fn file_import_diagnostics_and_graph_failures_are_reported_at_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module\n (export (func $f))\n (func $f (result i32) (i64.const 1)))",
    )
    .unwrap();
    std::fs::write(&main, "(module (import \"lib.wlt\" (func $f)) (func $main (export \"main\") (result i32) (call $f)))").unwrap();
    assert!(
        compile_file(&main)
            .err()
            .unwrap()
            .to_string()
            .contains("lib.wlt:3:")
    );
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module (import \"main.wlt\" (func $main)) (export (func $f)) (func $f (nop)))",
    )
    .unwrap();
    assert!(
        compile_file(&main)
            .err()
            .unwrap()
            .to_string()
            .contains("cycle")
    );
    std::fs::write(&main, "(module (import \"lib.wlt\"))").unwrap();
    assert!(compile_file(&main).is_err());
}

#[test]
fn file_types_share_identity_and_opaque_representations_stay_private() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("state.wlt"),
        r#"(module
      (export (opaque $State) (func $new) (func $get))
      (type $State (struct (field $n i32)))
      (func $new (result (ref $State)) (struct.new $State (i32.const 42)))
      (func $get (param $s (ref $State)) (result i32) (struct.get $State $n (local.get $s))))"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("facade.wlt"),
        r#"(module
      (import "state.wlt" (func $get $read)) (export (func $read)))"#,
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        r#"(module
      (import "state.wlt" (type $State $S) (func $new))
      (import "facade.wlt" (func $read))
      (func $main (export "main") (result i32) (call $read (call $new))))"#,
    )
    .unwrap();
    let compiled = compile_file(&main).unwrap();
    assert_eq!(
        compiled
            .ir
            .unwrap()
            .types
            .iter()
            .filter(|t| t.name.ends_with(".State"))
            .count(),
        1
    );
    std::fs::write(
        &main,
        r#"(module
      (import "state.wlt" (type $State $S) (func $new))
      (func $main (export "main") (result i32) (struct.get $S $n (call $new))))"#,
    )
    .unwrap();
    assert!(
        compile_file(&main)
            .err()
            .unwrap()
            .to_string()
            .contains("opaque")
    );
}

#[test]
fn wasm_export_can_name_a_statically_imported_function_without_a_wrapper() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module (export (func $f)) (func $f (result i32) (i32.const 42)))",
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        "(module (import \"lib.wlt\" (func $f $chosen)) (export \"answer\" (func $chosen)))",
    )
    .unwrap();
    let compiled = compile_file(&main).unwrap();
    assert!(compiled.wat.contains("(export \"answer\")"));
    assert_eq!(compiled.ir.unwrap().functions.len(), 1);
}

#[test]
fn generated_file_identities_are_hygienic_and_missing_exports_have_locations() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module (export (func $f)) (func $f (result i32) (i32.const 42)))",
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        r#"(module
      (namespace $wlt-file-0 (func $f (nop)))
      (import "lib.wlt" (func $f))
      (export "answer" (func $f)))"#,
    )
    .unwrap();
    assert!(compile_file(&main).is_ok());
    std::fs::write(
        &main,
        ";; 🦀\n(module\n (import \"lib.wlt\" (func $missing)))",
    )
    .unwrap();
    let error = compile_file(&main).err().unwrap().to_string();
    assert!(error.contains("main.wlt:3:"), "{error}");
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module\n (export (func $missing)))",
    )
    .unwrap();
    std::fs::write(&main, "(module (import \"lib.wlt\" (func $missing)))").unwrap();
    let error = compile_file(&main).err().unwrap().to_string();
    assert!(error.contains("lib.wlt:2:"), "{error}");
}

#[test]
fn file_modules_do_not_introduce_initialization_or_imported_test_roots() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(
        &main,
        "(module (import \"lib.wlt\" (func $f)) (export \"main\" (func $f)))",
    )
    .unwrap();
    for declaration in [
        "(start $init)",
        "(test \"not an imported API\" (i32.const 1))",
        "(export \"leak\" (func $f))",
    ] {
        std::fs::write(dir.path().join("lib.wlt"), format!("(module (export (func $f)) (func $init (nop)) (func $f (result i32) (i32.const 42)) {declaration})")).unwrap();
        assert!(compile_file(&main).is_err(), "{declaration}");
    }
}

#[test]
fn dotted_private_bindings_are_not_implicitly_reexported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.wlt"),
        "(module (export (func $f)) (func $f (result i32) (i32.const 42)))",
    )
    .unwrap();
    std::fs::write(dir.path().join("facade.wlt"), "(module (import \"lib.wlt\" (func $f $hidden.f)) (func $public (result i32) (call $hidden.f)) (export (func $public)))").unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(&main, "(module (import \"facade.wlt\" (func $hidden.f $leaked)) (export \"main\" (func $leaked)))").unwrap();
    assert!(compile_file(&main).is_err());
}
