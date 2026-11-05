use std::process::Command;

#[test]
fn native_guest_profile_driver_runs_the_actual_text_guest_and_checks_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("profile.wlt");
    let source = include_str!("../examples/io/text-filter.wlt").replacen(
        "(module",
        r#"(module
      (type $Unused (struct (field i64)))
      (func $unused (result (option i64)) (option.none i64))
      (import "wlt:host/event-loop@0.1.0" "yield" (func $unused-clock))"#,
        1,
    );
    std::fs::write(&input, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("profile-text")
        .arg(&input)
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,exceptions=y,function-references=y",
            "--preload",
        ])
        .arg(format!(
            "__component={}",
            dir.path().join("env.wasm").display()
        ))
        .arg("--preload")
        .arg(format!("guest={}", dir.path().join("core.wasm").display()))
        .args(["--invoke", "run"])
        .arg(dir.path().join("driver.wasm"))
        .args(["24", "32", "2", "0"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let values = String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 6);
    assert_eq!(values[0], 42, "non-empty lines over two rounds");
    assert!(
        values[1] <= 200,
        "fixed 24-row/two-round allocation budget: {}",
        values[1]
    );
    assert!(values[1] > 0 && values[2] > 0);
    assert_eq!(values[4], 65536);
    assert_eq!(values[5], 0, "no live ABI buffers");
    // Exercise the same driver with Unicode and an empty input stream.
    for (rows, width, mode, written) in [(24, 52, 1, 21), (0, 32, 0, 0)] {
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,exceptions=y,function-references=y",
                "--preload",
            ])
            .arg(format!(
                "__component={}",
                dir.path().join("env.wasm").display()
            ))
            .arg("--preload")
            .arg(format!("guest={}", dir.path().join("core.wasm").display()))
            .args(["--invoke", "run"])
            .arg(dir.path().join("driver.wasm"))
            .args([
                rows.to_string(),
                width.to_string(),
                "1".into(),
                mode.to_string(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let values = String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .map(|n| n.parse::<u64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values[0], written);
        assert_eq!(values[5], 0);
    }
}
