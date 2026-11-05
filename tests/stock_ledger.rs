use std::process::Command;

#[test]
fn stock_ledger_models_and_effect_boundaries_are_checked() {
    let loaded = gcwat::source::load("examples/apps/stock-ledger/main.gwat").unwrap();
    let compiled = gcwat::compile(loaded.text()).unwrap();
    for (name, host, suspends) in [
        ("$stock.add", false, false),
        ("$stock.take", false, false),
        ("$parse.command", false, false),
        ("$app.process", true, true),
    ] {
        let summary = compiled
            .analysis
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap();
        assert_eq!(
            (
                summary.semantic_effects.host,
                summary.semantic_effects.suspends
            ),
            (host, suspends)
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_gcwat"))
        .args(["test", "examples/apps/stock-ledger/model.test.gwat"])
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
        gcwat::check(&probe)
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
      (func $bad-pure (param $s (ref $stock.State)) (result (result i64 (ref $stock.Error))) (effects pure)
        (call $stock.add (local.get $s) (text.lit "a") (i64.const 1))))"#;
    assert!(
        gcwat::check(&pure)
            .unwrap_err()
            .to_string()
            .contains("effect contract")
    );
}
