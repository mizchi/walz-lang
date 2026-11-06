use std::path::Path;
use std::process::{Command, Output};

fn cli(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn declarations_check_types_names_and_keep_generated_functions_private() {
    let source = r#"(module
      (%use "wlt:core" (type $string))
      (%bench "unit" (nop))
      (%bench "number" (param $iteration i32) (result i64)
        (i64.extend_i32_u $iteration))
      (%bench "string" (result $string) (%string.lit "hello"))
      (%bench "vector" (result v128) (v128.const i32x4 1 2 3 4)))"#;
    let module = wlt::check(source).unwrap();
    assert_eq!(module.benches.len(), 4);
    assert!(module.tests.is_empty());
    let built = wlt::compile(source).unwrap();
    assert!(built.wat.contains("v128.const"));
    for (source, expected) in [
        ("(module (%bench \"\" (nop)))", "non-empty benchmark name"),
        ("(module (%bench $name (nop)))", "benchmark name string"),
        (
            "(module (%bench \"same\" (nop)) (%bench \"same\" (nop)))",
            "duplicate benchmark name",
        ),
        (
            "(module (%bench \"bad\" (result i64) (i32.const 1)))",
            "expected i64",
        ),
        ("(module (%bench \"bad\" (i32.const 1)))", "expected unit"),
        (
            "(module (%bench \"bad\" (param $i i64) (nop)))",
            "i32 iteration parameter",
        ),
        (
            "(module (%bench \"bad\" (param $i i32) (param $j i32) (nop)))",
            "one iteration parameter",
        ),
        (
            "(module (%bench \"bad\" (nop) (nop)))",
            "one benchmark body",
        ),
        (
            "(module (%bench \"a\" (nop)) (func $f (call $__bench0)))",
            "unknown function",
        ),
        ("(module (bench \"a\" (nop)))", "%bench"),
    ] {
        let error = wlt::check(source).unwrap_err().to_string();
        assert!(error.contains(expected), "{source}: {error}");
    }
    let error = wlt::check(
        r#"(module (%use "wlt:core" (type $box))
          (%bench "owned" (result (%inst $box i64)) (%box.new (i64.const 1))))"#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("owned"), "{error}");
}

#[test]
fn discovery_listing_filtering_and_tests_are_separate() {
    let directory = tempfile::tempdir().unwrap();
    let nested = directory.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(
        nested.join("sample_bench.wlt"),
        r#"(module
      (import "host" "read" (func $read (result i32)))
      (%bench "read one" (result i32) (call $read))
      (%bench "other" (nop))
      (%test "separate test" (i32.const 1)))"#,
    )
    .unwrap();
    std::fs::write(nested.join("ordinary.wlt"), "invalid").unwrap();
    std::fs::write(nested.join("ignored.test.wlt"), "(module)").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(directory.path())
        .args([
            "bench",
            ".",
            "nested/sample_bench.wlt",
            "--filter",
            "read",
            "--list",
        ])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    let stdout = text(&output);
    assert_eq!(stdout.matches("read one").count(), 1, "{stdout}");
    assert!(
        stdout.contains("1 benchmarks; 1 filtered out; 0 errors"),
        "{stdout}"
    );
    assert!(!stdout.contains("separate test"), "{stdout}");
    let default = cli(directory.path(), &["bench", "--list"]);
    assert!(default.status.success(), "{}", text(&default));
    let tests = cli(directory.path(), &["test", ".", "--list"]);
    assert!(
        text(&tests).contains("no tests matched"),
        "{}",
        text(&tests)
    );
}

#[test]
fn runs_resident_wasm_batches_and_emits_raw_samples_as_json() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("sample_bench.wlt"),
        r#"(module
      (%bench "index" (param $i i32) (result i64) (i64.extend_i32_u $i))
      (%bench "unit" (nop))
      (%bench "simd" (result v128) (v128.const i32x4 1 2 3 4))
      (%bench "gc" (result (ref $Item)) (struct.new $Item (i32.const 42)))
      (type $Item (struct (field i32))))"#,
    )
    .unwrap();
    let output = cli(
        directory.path(),
        &[
            "bench",
            "--samples",
            "3",
            "--iterations",
            "8",
            "--warmup-ms",
            "0",
            "--json",
        ],
    );
    assert!(output.status.success(), "{}", text(&output));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "wlt-bench-v1");
    let cases = report["benchmarks"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        assert_eq!(case["outcome"]["status"], "measured", "{case}");
        let summary = &case["outcome"]["summary"];
        assert_eq!(summary["iterations"], 8);
        assert_eq!(summary["samples_ns"].as_array().unwrap().len(), 3);
        assert!(summary["median_ns"].as_f64().unwrap() >= 0.0);
        assert!(summary["max_ns"].as_f64().unwrap() >= summary["min_ns"].as_f64().unwrap());
        assert!(summary["runtime"].as_str().unwrap().contains("Node.js"));
    }
}

#[test]
fn failures_timeouts_and_compile_errors_do_not_hide_peers() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("a_bench.wlt"),
        r#"(module
      (%bench "trap" (unreachable))
      (%bench "infinite" (%loop $forever (%recur $forever)))
      (%bench "after failure" (nop)))"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("b_bench.wlt"),
        "(module (%bench \"bad type\" (i64.const 1)))",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("c_bench.wlt"),
        "(module (%bench \"after compile error\" (nop)))",
    )
    .unwrap();
    let output = cli(
        directory.path(),
        &[
            "bench",
            ".",
            "--samples",
            "1",
            "--iterations",
            "1",
            "--warmup-ms",
            "0",
            "--timeout-ms",
            "1500",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    let stdout = text(&output);
    for expected in [
        "trap",
        "unreachable",
        "timed out",
        "after failure",
        "after compile error",
        "expected unit",
        "2 measured; 2 failed; 1 errors",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
}

#[test]
fn empty_selection_and_invalid_options_fail() {
    let directory = tempfile::tempdir().unwrap();
    let empty = cli(directory.path(), &["bench", "--list"]);
    assert_eq!(empty.status.code(), Some(1));
    assert!(text(&empty).contains("no benchmarks matched"));
    for args in [
        vec!["bench", "--samples", "0"],
        vec!["bench", "--samples", "10001"],
        vec!["bench", "--iterations", "0"],
        vec!["bench", "--iterations", "2147483648"],
        vec!["bench", "--sample-ms", "0"],
        vec!["bench", "--timeout-ms", "0"],
        vec!["bench", "--filter"],
        vec!["bench", "--unknown"],
    ] {
        assert_eq!(
            cli(directory.path(), &args).status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

#[test]
fn batch_wrapper_counts_iterations_and_retains_only_the_latest_gc_result() {
    let source = r#"(module
      (type $Item (struct (field i32)))
      (%bench "gc" (param $i i32) (result (ref $Item)) (struct.new $Item $i)))"#;
    let module = wlt::check(source).unwrap();
    let wasm = wlt::benchmark::compile(source, &module, 0).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bench.wasm");
    // A test-only observer checks the index passed to the body on every batch.
    let wat = wasmprinter_for_test(&wasm);
    let wasm = wat::parse_str(format!("{} (func (export \"index\") (result i32) (struct.get 0 0 (ref.as_non_null (global.get 0)))))", wat.trim_end().strip_suffix(')').unwrap())).unwrap();
    std::fs::write(&path, wasm).unwrap();
    let output = Command::new("node")
        .args([
            "--expose-gc",
            "--input-type=module",
            "-e",
            r#"
      import assert from 'node:assert/strict';
      import { readFile } from 'node:fs/promises';
      const { instance } = await WebAssembly.instantiate(await readFile(process.argv[1]));
      const api = instance.exports;
      assert.equal(api.__wlt_bench_sink.value, null);
      api.__wlt_bench_batch(8);
      assert.equal(api.index(), 7);
      const old = new WeakRef(api.__wlt_bench_sink.value);
      api.__wlt_bench_batch(3);
      assert.equal(api.index(), 2);
      api.__wlt_bench_batch(0);
      assert.equal(api.index(), 2);
      for (let i = 0; i < 30; i++) {
        await new Promise(resolve => setImmediate(resolve));
        globalThis.gc();
      }
      assert.equal(old.deref(), undefined, 'previous result must be collectable');
      const last = new WeakRef(api.__wlt_bench_sink.value);
      api.__wlt_bench_sink.value = null;
      for (let i = 0; i < 30; i++) {
        await new Promise(resolve => setImmediate(resolve));
        globalThis.gc();
      }
      assert.equal(last.deref(), undefined, 'clearing the sink releases its final result');
    "#,
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
}

fn wasmprinter_for_test(wasm: &[u8]) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bench.wasm");
    std::fs::write(&path, wasm).unwrap();
    let output = Command::new("wasm-tools")
        .arg("print")
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn selection_projects_unrelated_imports_and_supports_static_generic_libraries() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("lib.wlt"),
        r#"(module
      (%export (func $id))
      (func $id (%type.params $T) (param $value $T) (result $T) $value))"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("suite.wlt"),
        r#"(module
      (%use "./lib.wlt" (func $id))
      (import "host" "read" (func $read (result i32)))
      (%bench "needs host" (result i32) (call $read))
      (%bench "closed" (param $i i32) (result i64)
        (call (%inst $id i64) (i64.extend_i32_u $i))))"#,
    )
    .unwrap();
    let output = cli(
        directory.path(),
        &[
            "bench",
            "suite.wlt",
            "--filter",
            "closed",
            "--iterations",
            "4",
            "--samples",
            "2",
            "--warmup-ms",
            "0",
        ],
    );
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("1 measured; 0 failed; 0 errors; 1 filtered out"));
    let output = cli(
        directory.path(),
        &["bench", "suite.wlt", "--filter", "needs host"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output).contains("does not provide host imports"),
        "{}",
        text(&output)
    );
}

#[test]
fn unsupported_suspension_and_missing_runtime_are_reported_without_skipping_checks() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("suite.wlt"),
        r#"(module
      (%bench "await" (drop (%future.await i64 (%future.ready i64 (i64.const 1))))))"#,
    )
    .unwrap();
    let output = cli(directory.path(), &["bench", "suite.wlt"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output).contains("must be synchronous"),
        "{}",
        text(&output)
    );
    std::fs::write(
        directory.path().join("suite.wlt"),
        "(module (%bench \"valid\" (nop)))",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .current_dir(directory.path())
        .args(["bench", "suite.wlt"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output).contains("Node.js 24+"), "{}", text(&output));
    std::fs::write(
        directory.path().join("suite.wlt"),
        "(module (%bench \"valid\" (nop)) (%bench \"filtered out\" (i32.const 1)))",
    )
    .unwrap();
    let output = cli(
        directory.path(),
        &["bench", "suite.wlt", "--filter", "valid", "--list"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output).contains("expected unit"), "{}", text(&output));
}

#[test]
fn benchmark_generation_avoids_user_names_and_keeps_law_verification() {
    let source = r#"(module
      (func $user (export "__wlt_bench_0") (nop))
      (%law "identity" (param $x i32) (= $x $x))
      (%bench "generated" (nop)))"#;
    let module = wlt::check(source).unwrap();
    assert_ne!(module.benches[0].export, "__wlt_bench_0");
    assert!(wlt::benchmark::compile(source, &module, 0).is_ok());
    let false_law = source.replace("(= $x $x)", "(!= $x $x)");
    let module = wlt::check(&false_law).unwrap();
    assert!(wlt::benchmark::compile(&false_law, &module, 0).is_err());
    let sample = wlt::source::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/benchmarks/stdlib_v2_bench.wlt"),
    )
    .unwrap();
    let module = wlt::check(sample.text()).unwrap();
    assert_eq!(module.benches.len(), 4);
    for index in 0..module.benches.len() {
        wlt::benchmark::compile(sample.text(), &module, index).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn discovery_skips_symlinks_and_explicit_paths_and_separator_still_work() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let nested = directory.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(
        nested.join("sample_bench.wlt"),
        "(module (%bench \"one\" (nop)))",
    )
    .unwrap();
    symlink(directory.path(), nested.join("cycle")).unwrap();
    symlink(
        nested.join("sample_bench.wlt"),
        directory.path().join("linked_bench.wlt"),
    )
    .unwrap();
    let output = cli(
        directory.path(),
        &["bench", ".", "linked_bench.wlt", "--list"],
    );
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("1 benchmarks; 0 filtered out"));
    std::fs::write(
        directory.path().join("--sample.wlt"),
        "(module (%bench \"two\" (nop)))",
    )
    .unwrap();
    let output = cli(directory.path(), &["bench", "--list", "--", "--sample.wlt"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("two"));
}
