use std::process::Command;

#[test]
fn walz_cli_and_source_tests_work_outside_the_checkout() {
    let cli = env!("CARGO_BIN_EXE_walz");
    let version = Command::new(cli).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        concat!("walz ", env!("CARGO_PKG_VERSION"))
    );
    let help = Command::new(cli).output().unwrap();
    assert_eq!(help.status.code(), Some(2));
    let usage = String::from_utf8(help.stderr).unwrap();
    assert!(usage.contains("Usage: walz "), "{usage}");
    assert!(usage.contains("file.walz"), "{usage}");
    assert!(!usage.contains("gcwat"), "{usage}");
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("minimal.test.walz"),
        "(module (@test \"standalone\" (i32.const 1)))",
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
