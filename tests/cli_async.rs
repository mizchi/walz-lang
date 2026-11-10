use std::path::Path;
use std::process::{Command, Output};
fn cli(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn fixture(dir: &Path) {
    std::fs::write(dir.join("policy.json"), r#"{"imports":[{"module":"host","name":"tick","params":["i32"],"result":"i32","suspends":true,"capability":"tick"}]}"#).unwrap();
    std::fs::write(dir.join("host.mjs"), r#"import {appendFileSync} from 'node:fs'; export default ({signal})=>({imports:{host:{tick:async n=>{await new Promise(r=>setTimeout(r,1));return n+1;}}},async [Symbol.asyncDispose](){appendFileSync(new URL('./closed',import.meta.url),'x');}});"#).unwrap();
    std::fs::write(dir.join("suite.test.walz"), r#"(module
      (import "host" "tick" (func $tick (param $n i32) (result i32)))
      (@closure $Work (@capture $n i32) (result i32) (@effect host suspends) (call $tick $n))
      (@test "task" (@task.scope (result i32) (@let $t (@task.spawn i32 $Work (@closure.new $Work 40)) (= (@task.join i32 (@move $t)) 41))))
      (@test "false" (i32.const 0))
      (@test "after" (= (call $tick 41) 42)))"#).unwrap();
    std::fs::write(
        dir.join("suite_bench.walz"),
        r#"(module
      (import "host" "tick" (func $tick (param $n i32) (result i32)))
      (@bench "async" (param $i i32) (result i32) (call $tick $i)))"#,
    )
    .unwrap();
}
#[test]
fn async_cli_cases_share_checked_host_configuration_and_isolate_instances() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let out = cli(
        dir.path(),
        &[
            "test",
            "suite.test.walz",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--host",
            "host.mjs",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(
        text(&out).contains("2 passed; 1 failed; 0 errors"),
        "{}",
        text(&out)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("closed")).unwrap(),
        "xxx"
    );
    let out = cli(
        dir.path(),
        &[
            "bench",
            "suite_bench.walz",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--host",
            "host.mjs",
            "--samples",
            "2",
            "--iterations",
            "2",
            "--warmup-ms",
            "0",
            "--json",
        ],
    );
    assert!(out.status.success(), "{}", text(&out));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["benchmarks"][0]["outcome"]["status"], "measured");
    assert!(
        report["benchmarks"][0]["outcome"]["summary"]["runtime"]
            .as_str()
            .unwrap()
            .contains("Core+JSPI")
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("closed")).unwrap(),
        "xxxx"
    );
}
#[test]
fn async_deadlines_cancel_and_dispose_before_reporting_and_listing_does_not_load_host() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    std::fs::write(dir.path().join("host.mjs"),r#"import {appendFileSync} from 'node:fs'; export default {imports:{host:{tick:()=>{let done;const p=new Promise(r=>done=r);p[Symbol.asyncDispose]=()=>done(0);return p;}}},async [Symbol.asyncDispose](){await new Promise(r=>setTimeout(r,10));appendFileSync(new URL('./closed',import.meta.url),'x');}};"#).unwrap();
    let out = cli(
        dir.path(),
        &[
            "test",
            "suite.test.walz",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--host",
            "host.mjs",
            "--filter",
            "after",
            "--timeout-ms",
            "200",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("timed out"), "{}", text(&out));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("closed")).unwrap(),
        "x"
    );
    let out = cli(
        dir.path(),
        &[
            "bench",
            "suite_bench.walz",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--host",
            "host.mjs",
            "--timeout-ms",
            "200",
            "--samples",
            "1",
            "--iterations",
            "1",
            "--warmup-ms",
            "0",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("timed out"), "{}", text(&out));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("closed")).unwrap(),
        "xx"
    );
    let out = cli(
        dir.path(),
        &[
            "test",
            "suite.test.walz",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--host",
            "missing.mjs",
            "--list",
        ],
    );
    assert!(out.status.success(), "{}", text(&out));
    let out = cli(
        dir.path(),
        &["test", "suite.test.walz", "--host", "host.mjs"],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
}
