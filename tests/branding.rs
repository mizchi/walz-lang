use std::process::Command;

#[test]
fn wlt_cli_and_source_tests_work_outside_the_checkout() {
    let cli = env!("CARGO_BIN_EXE_wlt");
    let help = Command::new(cli).output().unwrap();
    assert_eq!(help.status.code(), Some(2));
    let usage = String::from_utf8(help.stderr).unwrap();
    assert!(usage.contains("Usage: wlt "), "{usage}");
    assert!(usage.contains("file.wlt"), "{usage}");
    assert!(!usage.contains("gcwat"), "{usage}");
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("minimal.test.wlt"),
        "(module (test \"standalone\" (i32.const 1)))",
    )
    .unwrap();
    let output = Command::new(cli)
        .current_dir(directory.path())
        .args(["test", "."])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed; 0 errors"));
}
