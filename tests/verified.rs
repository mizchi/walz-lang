use std::process::Command;

fn command(mode: &str, source: &str, path: Option<&str>) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.wlt");
    std::fs::write(&input, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_wlt"));
    command.args([mode]).arg(input);
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command.output().unwrap()
}

const GOOD: &str = r#"(module
  (func $main (export "main") (result i64) (%effects pure)
    (%let $x (%own.new i64 (i64.const 42))
      (%match (%own i64) (result i64) (%move $x) (case own ($n) (local.get $n))))))"#;

#[test]
fn contracts_are_inspectable_and_verified_execution_uses_the_checked_ir() {
    let output = command("contracts", GOOD, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["functions"][0]["name"], "$main");
    let output = command("verify", GOOD, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("verified"));
    let output = command("run-verified", GOOD, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}

#[test]
fn verified_mode_rejects_unknown_code_unchecked_exports_and_missing_kernel() {
    for (source, message) in [
        (
            "(module (func (export \"main\") (result i32) i32.const 42))",
            "typed core",
        ),
        (
            "(module (import \"host\" \"x\" (func $x)) (func $main (export \"main\") (%effects pure) (nop)))",
            "imports",
        ),
        (
            "(module (func $main (export \"main\") (nop)))",
            "effect contract",
        ),
        (
            "(module (func $main (export \"main\") (param $x (%own i64)) (%effects pure) (%own.close (%move $x))))",
            "owned exports",
        ),
    ] {
        let output = command("verify", source, None);
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = command("verify", GOOD, Some(""));
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("ownership kernel"));
    assert!(output.stdout.is_empty());
}

#[test]
fn independent_kernel_rejects_double_use_branch_mismatch_and_lost_loop_owners() {
    use serde_json::json;
    let take = |id| json!({"op":"take", "id":id});
    let seq = |items| json!({"op":"seq", "items":items});
    let nop = json!({"op":"nop"});
    let cases = [
        (json!({"op":"branch", "arms":[take(0),take(0)]}), true),
        (seq(vec![take(0), take(0)]), false),
        (json!({"op":"branch", "arms":[take(0),nop]}), false),
        (
            json!({"op":"loop", "owners":[1], "body":seq(vec![take(0),take(1),json!({"op":"again"})])}),
            false,
        ),
        (json!({"op":"scope", "id":1, "body":nop}), false),
    ];
    for (body, accepted) in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("contract.json");
        std::fs::write(
            &path,
            json!({"version":1,"functions":[{"name":"witness","inputs":[0],"body":body}]})
                .to_string(),
        )
        .unwrap();
        let output = Command::new("lean")
            .args([
                wlt::verified::LEAN_VERSION,
                "--run",
                wlt::verified::KERNEL_PATH,
            ])
            .arg(path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            accepted,
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn ready_delivery_runs_only_after_the_same_ownership_gate_and_exports_stay_protected() {
    let source = std::fs::read_to_string("examples/ready-delivery.wlt").unwrap();
    let output = command("run-verified", &source, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    for source in [
        "(module (func $f (export \"f\") (result (%future %unit)) (%effects pure) (%future.ready %unit)))",
        "(module (func $f (export \"f\") (param $s (%stream %unit)) (%effects pure) (%stream.close %unit (%move $s))))",
    ] {
        let output = command("verify", source, None);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("owned exports"));
    }
}

#[test]
fn independent_kernel_checks_graph_edges_contexts_and_terminals() {
    use serde_json::{Value, json};
    let n = |op: &str, id: usize, context: Value, next: Vec<usize>| json!({"op":op,"id":id,"context":context,"next":next});
    let good = vec![
        n("take", 0, json!([0]), vec![1]),
        n("finish", 0, json!([]), vec![]),
    ];
    let mut double = good.clone();
    double[1] = n("take", 0, json!([]), vec![2]);
    double.push(n("finish", 0, json!([]), vec![]));
    let mut mismatch = good.clone();
    mismatch[1]["context"] = json!([0]);
    let mut missing = good.clone();
    missing[1]["context"] = Value::Null;
    let cases = vec![
        (good, true),
        (double, false),
        (mismatch, false),
        (missing, false),
        (vec![n("finish", 0, json!([0]), vec![])], false),
        (vec![n("nop", 0, json!([0]), vec![])], false),
        (vec![n("nop", 0, json!([0]), vec![10])], false),
        (vec![n("nop", 0, json!([0, 0]), vec![0])], false),
        // A diverging path can keep an owner; every iteration still checks it.
        (vec![n("nop", 0, json!([0]), vec![0])], true),
    ];
    for (nodes, accepted) in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("graph.json");
        std::fs::write(&path,json!({"version":2,"functions":[{"name":"graph","inputs":[0],"graph":{"entry":0,"nodes":nodes}}]}).to_string()).unwrap();
        let output = Command::new("lean")
            .args([
                wlt::verified::LEAN_VERSION,
                "--run",
                wlt::verified::KERNEL_PATH,
            ])
            .arg(path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            accepted,
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
