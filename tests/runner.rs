use std::path::Path;
use std::process::{Command, Output};

fn cli(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn discovers_nested_test_files_deduplicates_and_accepts_nonzero_results() {
    let directory = tempfile::tempdir().unwrap();
    let tests = directory.path().join("tests/nested");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(
        tests.join("core.test.walz"),
        r#"(module
      (type $Option (@variant (none) (some i64)))
      (func $double (param $n i64) (result i64) (i64.mul (local.get $n) (i64.const 2)))
      (@test "numeric" (i64.eq (call $double (i64.const 21)) (i64.const 42)))
      (@test "enum" (@match $Option (result i32) (@variant.new $Option some (i64.const 42))
        (@case none () (i32.const 0)) (@case some ($n) (i64.eq (local.get $n) (i64.const 42)))))
      (@test "nonzero" (i32.const -1)))"#,
    )
    .unwrap();
    // Directory discovery must not treat ordinary modules as test files.
    std::fs::write(tests.join("ignored.walz"), "invalid source").unwrap();
    let output = cli(
        directory.path(),
        &["test", "tests", "tests/nested/core.test.walz"],
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout(&output).contains("3 passed; 0 failed; 0 errors"));
    let default = cli(directory.path(), &["test"]);
    assert!(default.status.success(), "{}", stdout(&default));
}

#[test]
fn failures_traps_and_compile_errors_do_not_hide_later_tests() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("a.test.walz"),
        r#"(module
      (@test "false" (i32.const 0))
      (@test "trap" (unreachable))
      (@test "after failure" (i32.const 1)))"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("b.test.walz"),
        "(module (@test \"wrong type\" (i64.const 1)))",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("c.test.walz"),
        "(module (@test \"after compile error\" (i32.const 1)))",
    )
    .unwrap();
    let output = cli(directory.path(), &["test", "."]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    for expected in [
        "returned 0",
        "unreachable",
        "expected i32, got i64",
        "after failure",
        "after compile error",
        "2 passed; 2 failed; 1 errors",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
}

#[test]
fn list_and_filter_work_without_a_runtime_and_empty_selection_is_an_error() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("suite.walz"),
        r#"(module
      (import "host" "read" (func $read (result i32)))
      (@test "enum one" (call $read))
      (@test "numeric" (i32.const 1))
      (@test "enum two" (i32.const 1)))"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(directory.path())
        .args(["test", "suite.walz", "--filter", "enum", "--list"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stdout(&output));
    let text = stdout(&output);
    assert!(
        text.contains("enum one") && text.contains("enum two") && !text.contains("numeric"),
        "{text}"
    );
    assert!(text.contains("2 tests; 1 filtered out"), "{text}");
    let output = cli(
        directory.path(),
        &["test", "suite.walz", "--filter", "missing"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stdout(&output).contains("no tests matched"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn infinite_tests_time_out_and_the_runner_continues() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("suite.walz"),
        r#"(module
      (@test "infinite" (@loop $forever (result i32) (@recur $forever)))
      (@test "after timeout" (i32.const 1)))"#,
    )
    .unwrap();
    let output = cli(
        directory.path(),
        &["test", "suite.walz", "--timeout-ms", "500"],
    );
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(
        text.contains("timed out") && text.contains("after timeout"),
        "{text}"
    );
    assert!(text.contains("1 passed; 1 failed; 0 errors"), "{text}");
}

#[test]
fn test_names_and_bodies_are_checked_without_exposing_generated_functions() {
    for (source, expected) in [
        (
            "(module (@test \"same\" (i32.const 1)) (@test \"same\" (i32.const 1)))",
            "duplicate test name",
        ),
        ("(module (@test \"\" (i32.const 1)))", "non-empty test name"),
        (
            "(module (@test $name (i32.const 1)))",
            "expected test name string",
        ),
        (
            "(module (@test \"bad\" (i64.const 1)))",
            "expected i32, got i64",
        ),
        (
            "(module (@test \"bad\" (local.get $hidden)))",
            "unbound local",
        ),
        ("(module (@test \"bad\" (nop)))", "expected i32, got unit"),
        (
            "(module (@test \"bad\" (i32.const 1) (i32.const 1)))",
            "expected (test",
        ),
        (
            "(module (@test \"hidden\" (i32.const 1)) (func $f (result i32) (call $__test0)))",
            "unknown function",
        ),
    ] {
        let error = walz::check(source).unwrap_err().to_string();
        assert!(error.contains(expected), "{source}\n{error}");
    }
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("collision.walz"),
        r#"(module
      (func $user (export "__walz_test_0") (result i32) (i32.const 0))
      (@test "generated export" (i32.const 1)))"#,
    )
    .unwrap();
    let output = cli(directory.path(), &["test", "collision.walz"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn reports_empty_suites_missing_runtime_and_invalid_arguments() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("empty.walz"), "(module)").unwrap();
    let output = cli(directory.path(), &["test", "empty.walz"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("no tests matched"));
    for args in [
        vec!["test", "--timeout-ms", "0"],
        vec!["test", "--filter"],
        vec!["test", "--unknown"],
    ] {
        assert_eq!(cli(directory.path(), &args).status.code(), Some(2));
    }
    std::fs::write(
        directory.path().join("one.walz"),
        "(module (@test \"one\" (i32.const 1)))",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(directory.path())
        .args(["test", "one.walz"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("could not start wasmtime"));
}

#[test]
fn tests_keep_explicit_gc_effects_and_a_closed_call_graph() {
    let source = r#"(module
      (type $Cell (struct (field $value (mut i64))))
      (@closure $Add (@capture $base i64) (param $n i64) (result i64)
        (i64.add (local.get $base) (local.get $n)))
      (@test "immutable capture"
        (@let $f (@closure.new $Add (i64.const 40))
          (i64.eq (@closure.call $Add (local.get $f) (i64.const 2)) (i64.const 42))))
      (@test "explicit mutation"
        (@let $cell (struct.new $Cell (i64.const 0))
          (block (result i32)
            (struct.set $Cell $value (local.get $cell) (i64.const 42))
            (i64.eq (struct.get $Cell $value (local.get $cell)) (i64.const 42))))))"#;
    let module = walz::check(source).unwrap();
    let analysis = walz::analysis::analyze(&module);
    let capture = &analysis.functions[module.tests[0].function.0];
    assert_eq!(capture.calls, ["$Add"]);
    assert!(!capture.effects.host && !capture.effects.writes && !capture.effects.reads);
    assert!(capture.effects.allocates);
    let mutation = &analysis.functions[module.tests[1].function.0];
    assert!(mutation.effects.writes && mutation.effects.reads && mutation.effects.allocates);
    assert!(!mutation.effects.host);
    for test in &module.tests {
        let function = &module.functions[test.function.0];
        assert!(function.params.is_empty() && function.captures.is_empty());
        assert_eq!(module.type_name(&function.result), "i32");
        assert!(source[test.span.start..test.span.end].starts_with("(@test"));
    }
}

#[test]
fn runs_committed_language_test_suites() {
    let output = cli(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &["test", "tests/programs"],
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout(&output).contains("21 passed; 0 failed; 0 errors"));
}
