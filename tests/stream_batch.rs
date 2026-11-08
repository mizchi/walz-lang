#[test]
fn batch_primitive_preserves_ready_order_and_affine_tail_in_all_backends() {
    let source = include_str!("component-fixtures/stream-batch.wlt");
    let built = wlt::compile(source).unwrap();
    assert!(built.wat.contains("__read_batch_"));
    wlt::component::compile(source).unwrap();
    wlt::component::core_js::compile(source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("suite.wasm");
    std::fs::write(&path, built.wasm).unwrap();
    let result = std::process::Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            "local",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    // A used stream must still be consumed exactly once after returning a tail.
    let bad = source.replace("(%stream.close i32 (%move $tail))", "(nop)");
    assert!(wlt::check(&bad).is_err());
}
