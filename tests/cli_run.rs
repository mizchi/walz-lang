use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[cfg(unix)]
#[test]
fn interrupting_the_parent_waits_for_node_guest_and_host_disposal() {
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let ready = dir.path().join("ready");
    let closed = dir.path().join("closed");
    let closing = dir.path().join("closing");
    std::fs::write(
        dir.path().join("main.wlt"),
        r#"(module
      (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $n i32)))
      (func $main (export "main") (%effect host suspends) (call $sleep 10000)))"#,
    )
    .unwrap();
    let host = format!(
        r#"import {{writeFileSync}} from 'node:fs';
      export default {{eventLoop:{{sleep:()=>{{let done;const promise=new Promise(r=>{{done=r;}});const timer=setInterval(()=>{{}},1000);
        promise[Symbol.asyncDispose]=()=>{{clearInterval(timer);done();}};
        writeFileSync({},String(process.pid));return promise;
      }}}},async [Symbol.asyncDispose](){{writeFileSync({},'closing');await new Promise(r=>setTimeout(r,200));writeFileSync({},'closed');}}}};"#,
        serde_json::to_string(&ready).unwrap(),
        serde_json::to_string(&closing).unwrap(),
        serde_json::to_string(&closed).unwrap()
    );
    std::fs::write(dir.path().join("host.mjs"), host).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(dir.path())
        .args(["run", "--target", "core-js", "--host", "host.mjs"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !ready.exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "guest did not start");
    Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    while !closing.exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    // A repeated interrupt during host disposal must still wait for cleanup.
    Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > until {
            child.kill().unwrap();
            panic!("parent did not acknowledge cancellation");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Retire our own child if testing the broken parent leaves Node behind.
    if !closed.exists() {
        let pid = std::fs::read_to_string(&ready).unwrap();
        Command::new("kill")
            .args(["-INT", pid.trim()])
            .status()
            .unwrap();
    }
    assert_eq!(status.code(), Some(130));
    assert!(
        closed.exists(),
        "parent exited before asynchronous host disposal"
    );
}

fn run(directory: &Path, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn native_run_selects_an_export_passes_arguments_and_supports_exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.wlt"),
        r#"(module
      (func $main (export "main") (result i32) 7)
      (func $add (export "add") (param $a i64) (param $b i64) (result i64) (+ $a $b)))"#,
    )
    .unwrap();
    assert_eq!(success(run(dir.path(), &["run"], "")).trim(), "7");
    assert_eq!(
        success(run(
            dir.path(),
            &["run", "--invoke", "add", "--", "40", "2"],
            ""
        ))
        .trim(),
        "42"
    );
    let status = run(dir.path(), &["run", "--exit-code"], "");
    assert_eq!(status.status.code(), Some(7));
    assert!(status.stdout.is_empty());
}

#[test]
fn jspi_run_works_with_arguments_and_gc_values_across_suspension() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let file = root.join("tests/component-fixtures/event-loop.wlt");
    let out = run(
        root,
        &[
            "run",
            file.to_str().unwrap(),
            "--target",
            "core-js",
            "--",
            "0",
        ],
        "",
    );
    assert_eq!(success(out).trim(), "42");
    let out = run(
        root,
        &[
            "run",
            file.to_str().unwrap(),
            "--target",
            "core-js",
            "--invoke",
            "sum-ticks",
            "--",
            "5",
            "0",
        ],
        "",
    );
    assert_eq!(success(out).trim(), "10");
}

#[test]
fn text_io_requires_explicit_capabilities_and_leaves_stdout_for_program_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/io/text-filter.wlt");
    let denied = run(
        dir.path(),
        &["run", source.to_str().unwrap(), "--target", "core-js"],
        "",
    );
    assert_eq!(denied.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&denied.stderr).contains("capability"));
    let out = run(
        dir.path(),
        &[
            "run",
            source.to_str().unwrap(),
            "--target",
            "core-js",
            "--stdio",
        ],
        "hello\n\n日本語\n",
    );
    assert_eq!(success(out), "hello\n日本語\n");
    std::fs::write(dir.path().join("input.txt"), "file\n\n").unwrap();
    let out = run(
        dir.path(),
        &[
            "run",
            source.to_str().unwrap(),
            "--target",
            "core-js",
            "--stdio",
            "--input",
            "input.txt",
            "--output",
            "output.txt",
        ],
        "",
    );
    assert_eq!(success(out), "");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("output.txt")).unwrap(),
        "file\n"
    );
    let aliased = run(
        dir.path(),
        &[
            "run",
            source.to_str().unwrap(),
            "--target",
            "core-js",
            "--stdio",
            "--input",
            "input.txt",
            "--output",
            "./input.txt",
        ],
        "",
    );
    assert_eq!(aliased.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("input.txt")).unwrap(),
        "file\n\n"
    );
}

#[test]
fn cli_arguments_are_available_through_a_typed_host_binding() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.wlt"),
        r#"(module
      (%use "wlt:core" (type $list) (type $string))
      (import "wlt:host/cli@0.1.0" "arguments" (func $args (result (%inst $list $string))))
      (func $main (export "main") (result i32) (%effect host)
        (%list.len $string (call $args))))"#,
    )
    .unwrap();
    assert_eq!(
        success(run(
            dir.path(),
            &["run", "--target", "core-js", "--", "hello", "世界"],
            ""
        ))
        .trim(),
        "2"
    );
}

#[test]
fn run_usage_errors_are_distinct_from_guest_failures() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["run", "--target", "unknown"],
        vec!["run", "--invoke"],
        vec!["run", "--unknown"],
        vec!["run", "--input", "input.txt"],
        vec!["run", "--stdio"],
    ] {
        assert_eq!(
            run(dir.path(), &args, "").status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

#[test]
fn explicit_host_modules_use_the_checked_policy_and_bigint_arguments() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.wlt"),
        r#"(module
      (import "example:host/math@1.0.0" "double" (func $double (param $value i64) (result i64)))
      (func $main (export "main") (param $n i64) (result i64) (%effect host) (call $double $n)))"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("policy.json"), r#"{"imports":[{"module":"example:host/math@1.0.0","name":"double","params":["i64"],"result":"i64","suspends":false,"capability":"math"}]}"#).unwrap();
    std::fs::write(dir.path().join("host.mjs"), "export default () => ({ imports: { 'example:host/math@1.0.0': { double: n => n * 2n } } });").unwrap();
    let output = run(
        dir.path(),
        &[
            "run",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--host",
            "host.mjs",
            "--",
            "\"21n\"",
        ],
        "",
    );
    assert_eq!(success(output).trim(), "42");
    let denied = run(
        dir.path(),
        &[
            "run",
            "--target",
            "core-js",
            "--bindings",
            "policy.json",
            "--",
            "\"21n\"",
        ],
        "",
    );
    assert_eq!(denied.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&denied.stderr).contains("capability"));
}

#[test]
fn cli_argument_contract_cannot_be_retyped_or_overridden() {
    let source = r#"(module (import "wlt:host/cli@0.1.0" "arguments" (func $args (result i32))))"#;
    assert!(wlt::check(source).is_err());
    assert!(wlt::bindings::BindingPolicy::parse(r#"{"imports":[{"module":"wlt:host/cli@0.1.0","name":"arguments","params":[],"result":"i32","suspends":false,"capability":"lie"}]}"#).is_err());
}

#[test]
fn filesystem_value_adapter_requires_named_file_grants_and_closes_both_endpoints() {
    let dir = tempfile::tempdir().unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/wasi-files");
    for name in ["main.wlt", "batch.wlt", "filesystem.wit", "policy.json"] {
        std::fs::copy(example.join(name), dir.path().join(name)).unwrap();
    }
    success(run(
        dir.path(),
        &["bind", "filesystem.wit", "policy.json", "generated"],
        "",
    ));
    std::fs::write(dir.path().join("input.bin"), [65, 66]).unwrap();
    let output = run(
        dir.path(),
        &[
            "run",
            "--target",
            "core-js",
            "--bindings",
            "generated/bindings.json",
            "--read-file",
            "input=input.bin",
        ],
        "",
    );
    assert_eq!(success(output).trim(), "131");
    let written = run(
        dir.path(),
        &[
            "run",
            "--target",
            "core-js",
            "--bindings",
            "generated/bindings.json",
            "--invoke",
            "write",
            "--exit-code",
            "--write-file",
            "output=output.bin",
        ],
        "",
    );
    assert_eq!(success(written), "");
    assert_eq!(
        std::fs::read(dir.path().join("output.bin")).unwrap(),
        [65, 66]
    );
    let alias = run(
        dir.path(),
        &[
            "run",
            "--target",
            "core-js",
            "--bindings",
            "generated/bindings.json",
            "--read-file",
            "input=input.bin",
            "--write-file",
            "output=./input.bin",
        ],
        "",
    );
    assert_eq!(alias.status.code(), Some(1));
    assert_eq!(
        std::fs::read(dir.path().join("input.bin")).unwrap(),
        [65, 66]
    );
}
