use std::process::Command;

#[test]
fn log_statistics_modules_have_checked_effect_boundaries_and_native_model_tests() {
    let loaded = wlt::source::load("examples/apps/log-stats/main.wlt").unwrap();
    let compiled = wlt::compile(loaded.text()).unwrap();
    assert!(!compiled.analysis.opaque);
    for (name, reads, writes, host, suspends) in [
        ("$parse.line", false, false, false, false),
        ("$stats.consume", true, true, false, false),
        ("$report.write", true, true, true, true),
    ] {
        let effects = &compiled
            .analysis
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .semantic_effects;
        assert_eq!(
            (
                effects.reads,
                effects.writes,
                effects.host,
                effects.suspends
            ),
            (reads, writes, host, suspends),
            "{name}"
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "examples/apps/log-stats/model.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("7 passed; 0 failed; 0 errors"));
}

#[test]
fn native_log_workload_checks_all_rows_and_resets_state_between_rounds() {
    let loaded = wlt::source::load("examples/apps/log-stats/benchmark.wlt").unwrap();
    let compiled = wlt::compile(loaded.text()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("compute.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    for (rows, rounds, expected) in [(0, 2, 0), (6, 1, 73), (7, 2, 172), (1200, 3, 43800)] {
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                "run",
            ])
            .arg(&path)
            .arg(rows.to_string())
            .arg(rounds.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected.to_string()
        );
    }
}
