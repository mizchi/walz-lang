use std::process::Command;

#[test]
fn stock_ledger_models_and_effect_boundaries_are_checked() {
    let loaded = wlt::source::load("examples/apps/stock-ledger/main.wlt").unwrap();
    let compiled = wlt::compile(loaded.text()).unwrap();
    for (name, host, suspends) in [
        (".add", false, false),
        (".take", false, false),
        (".command", false, false),
        (".process", true, true),
    ] {
        let summary = compiled
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
            (host, suspends)
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "examples/apps/stock-ledger/model.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("7 passed; 0 failed; 0 errors"));
    let probe = loaded.text();
    // The same state reference may be passed around, but its fields stay private.
    let probe = probe.trim_end().strip_suffix(')').unwrap().to_owned()
        + " (func $forge (param $s (ref $stock.State)) (struct.set $stock.State $size (local.get $s) (i32.const 0))))";
    assert!(
        wlt::check(&probe)
            .unwrap_err()
            .to_string()
            .contains("opaque")
    );
    let pure = loaded
        .text()
        .trim_end()
        .strip_suffix(')')
        .unwrap()
        .to_owned()
        + r#"
      (func $bad-pure (param $s (ref $stock.State)) (result (%result i64 (ref $stock.Error))) (%effect pure)
        (call $stock.add (local.get $s) (%string.lit "a") (i64.const 1))))"#;
    assert!(
        wlt::check(&pure)
            .unwrap_err()
            .to_string()
            .contains("effect contract")
    );
}
