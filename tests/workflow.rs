use std::process::Command;

#[test]
fn workflow_has_checked_effect_boundaries_and_runs_its_model_tests() {
    let loaded = wlt::source::load("examples/apps/workflow/main.wlt").unwrap();
    let module = wlt::compile(loaded.text()).unwrap();
    assert!(!module.analysis.opaque);
    for (name, host, suspends) in [
        (".command", false, false),
        (".add", false, false),
        (".run", false, false),
        (".plan", true, true),
        (".advance", true, true),
    ] {
        let summary = module
            .analysis
            .functions
            .iter()
            .find(|f| {
                f.name
                    .split_once('.')
                    .is_some_and(|(_, member)| member == name.trim_start_matches('.'))
            })
            .unwrap();
        assert_eq!(
            (
                summary.semantic_effects.host,
                summary.semantic_effects.suspends
            ),
            (host, suspends),
            "{name}"
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "examples/apps/workflow/model.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("12 passed; 0 failed; 0 errors"));
    let input = std::fs::read_to_string("examples/apps/workflow/main.wlt").unwrap();
    let prefix = input.trim_end().strip_suffix(')').unwrap();
    let check_probe = |text: &str| {
        let path = tempfile::Builder::new()
            .suffix(".wlt")
            .tempfile_in("examples/apps/workflow")
            .unwrap();
        std::fs::write(path.path(), text).unwrap();
        let loaded = wlt::source::load(path.path()).unwrap();
        wlt::check(loaded.text())
    };
    let probe = format!(
        "{prefix} (func $forge (param $g (ref $graph.Graph)) (result i32) (struct.get $graph.Graph $size (local.get $g))))"
    );
    assert!(
        check_probe(&probe)
            .unwrap_err()
            .to_string()
            .contains("opaque")
    );
    let probe = format!(
        "{prefix} (%use \"./planner.wlt\" (type $Run $planner.Run)) (func $impure (param $g (ref $graph.Graph)) (result (%result (ref $planner.Run) (ref $domain.Error))) (%effect pure) (call $planner.run (local.get $g) (i32.const 2))))"
    );
    assert!(
        check_probe(&probe)
            .unwrap_err()
            .to_string()
            .contains("effect contract")
    );
}

#[test]
fn workflow_stress_workloads_run_natively_and_keep_wide_timestamps() {
    let loaded = wlt::source::load("examples/apps/workflow/benchmark.wlt").unwrap();
    let compiled = wlt::compile(loaded.text()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("compute.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    for (export, args, expected) in [
        ("chain", vec!["0", "2"], "0"),
        ("chain", vec!["2200", "3"], "6600000000"),
        ("edge-limit", vec![], "1"),
    ] {
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                export,
            ])
            .arg(&path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn workflow_ready_queue_handles_ordering_and_interleaved_updates() {
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "examples/apps/workflow/queue.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("3 passed; 0 failed; 0 errors"));
}
