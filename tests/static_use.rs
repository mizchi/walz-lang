use std::process::Command;

#[test]
fn relative_file_uses_select_exports_and_aliases_without_namespace_declarations() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("math.walz"),
        r#"(module
      (@export (func $answer))
      (func $private (result i64) 40)
      (func $answer (result i64) (+ (call $private) 2)))"#,
    )
    .unwrap();
    let path = directory.path().join("main.walz");
    std::fs::write(
        &path,
        r#"(module
      (@use "walz:core" (type $result))
      (@use "./math.walz" (func $answer $calculate))
      (func $main (export "main") (result i64)
        (@match (@result.ok i64 i32 (call $calculate))
          (@case ok ($n) $n) (@case err ($code) 0))))"#,
    )
    .unwrap();
    let loaded = walz::source::load(&path).unwrap();
    let compiled = walz::compile(loaded.text()).unwrap();
    let wasm = directory.path().join("main.wasm");
    std::fs::write(&wasm, compiled.wasm).unwrap();
    let output = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y",
            "--invoke",
            "main",
        ])
        .arg(wasm)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}

#[test]
fn namespaces_atom_uses_old_imports_and_ambiguous_file_names_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.walz");
    for (source, code) in [
        ("(module (@namespace $math))", "E_WALZ_EXTENSION"),
        ("(module (@use $math (func $f)))", "E_USE_PATH"),
        ("(module (@use \"math.walz\" (func $f)))", "E_USE_PATH"),
        (
            "(module (@import \"walz:core\" (type $result)))",
            "E_WALZ_EXTENSION",
        ),
    ] {
        let error = walz::check(source).unwrap_err();
        assert_eq!(error.diagnostic().code, code, "{source}: {error}");
        std::fs::write(&path, source).unwrap();
        let error = walz::source::load(&path).err().unwrap();
        assert_eq!(error.diagnostic().code, code, "{source}: {error}");
    }
    let native = r#"(module (import "./math.walz" "answer" (func $answer (result i32))))"#;
    assert_eq!(walz::compile(native).unwrap().wat, native);
}
