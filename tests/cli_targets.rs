use std::path::Path;
use std::process::{Command, Output};
fn cli(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn success(out: Output) {
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn core_js_build_is_a_standalone_runtime_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/component-fixtures/event-loop.wlt");
    success(cli(
        dir.path(),
        &[
            "build",
            source.to_str().unwrap(),
            "--target",
            "core-js",
            "-o",
            "output bundle",
        ],
    ));
    for name in [
        "index.js",
        "core.wasm",
        "memory.wasm",
        "core.mjs",
        "host.mjs",
        "tasks.mjs",
        "resources.mjs",
        "bindings.mjs",
        "manifest.json",
        "package.json",
    ] {
        assert!(
            dir.path().join("output bundle").join(name).is_file(),
            "{name}"
        );
    }
    let output = Command::new("node").current_dir(dir.path()).args(["--input-type=module", "-e", "const {instantiate}=await import('./output bundle/index.js'); const {api}=await instantiate(); console.log(String(await api.main(0))); "]).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "42",
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success());
}

#[test]
fn build_targets_have_distinct_default_outputs_and_reject_unknown_targets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.wlt"),
        "(module (func $main (export \"main\") (result i32) (%effect pure) 42))",
    )
    .unwrap();
    for target in ["component", "core", "core-js"] {
        success(cli(dir.path(), &["build", "--target", target]));
    }
    assert!(dir.path().join("target/main.wasm").is_file());
    assert!(dir.path().join("target/main-js/index.js").is_file());
    let component = std::fs::read(dir.path().join("target/main.component.wasm")).unwrap();
    assert!(wasmparser::Parser::is_component(&component));
    assert_eq!(
        cli(dir.path(), &["build", "--target", "unknown"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        cli(dir.path(), &["build", "--target"]).status.code(),
        Some(2)
    );
}

#[test]
fn bundled_outputs_cannot_overwrite_the_source_or_its_links() {
    let dir = tempfile::tempdir().unwrap();
    let source = "(module (func $main (export \"main\") (result i32) (%effect pure) 42))";
    std::fs::write(dir.path().join("core.wasm"), source).unwrap();
    assert_eq!(
        cli(
            dir.path(),
            &["build", "core.wasm", "--target", "core-js", "-o", "."]
        )
        .status
        .code(),
        Some(1)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("core.wasm")).unwrap(),
        source
    );
}

#[test]
fn component_metadata_cannot_overwrite_an_aliased_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = "(module (func $main (export \"main\") (result i32) (%effect pure) 42))";
    std::fs::write(dir.path().join("main.wlt"), source).unwrap();
    std::fs::hard_link(
        dir.path().join("main.wlt"),
        dir.path().join("app.wasm.json"),
    )
    .unwrap();
    let result = cli(
        dir.path(),
        &[
            "build",
            "main.wlt",
            "--target",
            "component",
            "-o",
            "app.wasm",
        ],
    );
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("main.wlt")).unwrap(),
        source
    );
    assert!(!dir.path().join("app.wasm").exists());
}
