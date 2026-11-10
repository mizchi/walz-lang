use serde_json::{Value, json};
use std::process::{Command, Output};

const COPY: &str = r#"(module
  (@use "walz:core" (type $bytes))
  (func $copy (param $input $bytes) (result $bytes)
    (@bytes.slice $input 0 (@bytes.len $input)))
  (func $forward (param $input $bytes) (result $bytes) (call $copy $input))
  (func $main (export "main") (param $input $bytes) (param $limit i32) (result i32)
    (@loop $again (bind $i i32 0) (bind $sum i32 0) (result i32)
      (if (result i32) (= $i $limit)
        (then $sum)
        (else (@let $part (call $forward $input)
          (@recur $again (+ $i 1) (+ $sum (@bytes.get $part 0)))))))))"#;

const HOST: &str = r#"(module
  (type $Node (struct (field i32)))
  (import "host" "save" (func $save (param $node (ref $Node))))
  (func $main (export "main") (param $limit i32) (result i32)
    (@loop $again (bind $i i32 0) (result i32)
      (if (result i32) (= $i $limit)
        (then $i)
        (else (block (result i32)
          (@let $node (struct.new $Node $i) (call $save $node))
          (@recur $again (+ $i 1))))))))"#;

fn check(source: &str, flags: &[&str]) -> Output {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.walz");
    std::fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("check")
        .args(flags)
        .arg(path)
        .output()
        .unwrap()
}

fn report(source: &str) -> Value {
    let output = check(source, &["--warn-performance", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true, "{value}");
    value
}

#[test]
fn loop_copy_warnings_follow_checked_calls_and_do_not_fail_the_check() {
    let value = report(COPY);
    let warnings = value["performance"]["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{value}");
    assert_eq!(warnings[0]["code"], "W_OPT_LOOP_COPY");
    assert_eq!(warnings[0]["primary"]["location"]["line"], 10);
    assert!(warnings[0]["message"].as_str().unwrap().contains("copy"));
    assert!(!warnings[0]["help"].as_array().unwrap().is_empty());
    assert_eq!(value["diagnostics"], json!([]));
    let text = check(COPY, &["--warn-performance"]);
    assert!(text.status.success());
    assert!(String::from_utf8_lossy(&text.stderr).contains("warning[W_OPT_LOOP_COPY]"));
    let ordinary = check(COPY, &["--json"]);
    let ordinary: Value = serde_json::from_slice(&ordinary.stdout).unwrap();
    assert!(ordinary.get("performance").is_none());
}

#[test]
fn host_escape_warns_at_the_allocation_but_scalar_host_arguments_do_not() {
    let value = report(HOST);
    let warnings = value["performance"]["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{value}");
    assert_eq!(warnings[0]["code"], "W_OPT_LOOP_HOST_ESCAPE");
    assert_eq!(warnings[0]["primary"]["location"]["line"], 9);
    let scalar = HOST
        .replace("(param $node (ref $Node))", "(param $n i32)")
        .replace(
            "(call $save $node)",
            "(call $save (struct.get $Node 0 $node))",
        );
    assert_eq!(report(&scalar)["performance"]["warnings"], json!([]));
}

#[test]
fn cold_initializers_dead_functions_and_scalar_mutability_are_quiet() {
    let initializer = COPY
        .replace(
            "(bind $sum i32 0)",
            "(bind $sum i32 (@bytes.len (call $forward $input)))",
        )
        .replace(
            "(call $forward $input)\n          (@recur",
            "$input\n          (@recur",
        );
    assert_eq!(report(&initializer)["performance"]["warnings"], json!([]));
    assert_eq!(
        report(&COPY.replace("(export \"main\")", ""))["performance"]["warnings"],
        json!([])
    );
    let scalar = "(module (type $Cell (struct (field (mut i32)))) (func $main (export \"main\") (result i32) (@let $c (struct.new $Cell 42) (struct.get $Cell 0 $c))))";
    assert_eq!(report(scalar)["performance"]["warnings"], json!([]));
}

#[test]
fn native_wat_coverage_and_errors_remain_explicit() {
    let native = report("(module (func (export \"main\") (result i32) i32.const 42))");
    assert_eq!(native["performance"]["coverage"], "opaque_wat");
    assert_eq!(native["performance"]["warnings"], json!([]));
    let bad = check(
        "(module (func $main (result i64) (i32.const 42)))",
        &["--warn-performance", "--json"],
    );
    assert!(!bad.status.success());
    let bad: Value = serde_json::from_slice(&bad.stdout).unwrap();
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["diagnostics"][0]["code"], "E_TYPE_MISMATCH");
    assert!(bad.get("performance").is_none());
}

#[test]
fn list_copies_nested_loop_initializers_and_unreachable_branches_are_distinguished() {
    let list = COPY
        .replace("(type $bytes)", "(type $list)")
        .replace("$input $bytes", "$input (@inst $list i32)")
        .replace("(result $bytes)", "(result (@inst $list i32))")
        .replace("@bytes.slice", "@list.slice i32")
        .replace("@bytes.len", "@list.len i32")
        .replace("@bytes.get", "@list.get i32");
    assert_eq!(
        report(&list)["performance"]["warnings"][0]["code"],
        "W_OPT_LOOP_COPY"
    );
    let nested = COPY.replace("(call $forward $input)\n          (@recur",
        "(@loop $inner (bind $warm $bytes (call $forward $input)) (result $bytes) $warm)\n          (@recur");
    assert_eq!(
        report(&nested)["performance"]["warnings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let unreachable = COPY.replace("(call $forward $input)\n          (@recur",
        "(if (result $bytes) (i32.const 0) (then (call $forward $input)) (else $input))\n          (@recur");
    assert_eq!(report(&unreachable)["performance"]["warnings"], json!([]));
}

#[test]
fn warnings_preserve_module_identities_and_original_import_locations() {
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("copy.walz");
    let root = dir.path().join("main.walz");
    let library = "(module (@use \"walz:core\" (type $bytes)) (@export (func $copy)) (func $copy (param $input $bytes) (result $bytes) (@bytes.slice $input 0 (@bytes.len $input))))";
    std::fs::write(&part, library).unwrap();
    let start = COPY.find("  (func $copy").unwrap();
    let end = COPY.find("  (func $forward").unwrap();
    let source = format!(
        "{}  (@use \"./copy.walz\" (func $copy $aliased))\n{}",
        &COPY[..start],
        &COPY[end..]
    )
    .replace("(call $copy $input)", "(call $aliased $input)");
    std::fs::write(&root, source).unwrap();
    let loaded = walz::source::load(&root).unwrap();
    let before = walz::compile(loaded.text()).unwrap();
    let module = walz::check(loaded.text()).unwrap();
    let identity = serde_json::to_value(&module).unwrap();
    let report = walz::performance::analyze(loaded.text(), &module).located(&loaded);
    assert_eq!(serde_json::to_value(&module).unwrap(), identity);
    assert_eq!(before.wasm, walz::compile(loaded.text()).unwrap().wasm);
    assert_eq!(report.warnings.len(), 1);
    let warning = &report.warnings[0];
    assert_eq!(
        warning.primary.as_ref().unwrap().location.path.as_deref(),
        Some(root.canonicalize().unwrap().as_path())
    );
    let related = &warning.related[0];
    assert_eq!(
        related.location.path.as_deref(),
        Some(part.canonicalize().unwrap().as_path())
    );
    assert!(library[related.span.start..related.span.end].contains("@bytes.slice"));

    let uri = walz::lsp::file_uri(&root).unwrap();
    let part_uri = walz::lsp::file_uri(&part).unwrap();
    let mut session = walz::lsp::Session::new(Default::default());
    session.handle(json!({"id":1,"method":"initialize","params":{"initializationOptions":{"performanceWarnings":true}}}));
    let opened = session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":std::fs::read_to_string(&root).unwrap()}}}));
    let diagnostics = &opened[0]["params"]["diagnostics"];
    assert_eq!(diagnostics[0]["code"], "W_OPT_LOOP_COPY");
    assert_eq!(
        diagnostics[0]["relatedInformation"][0]["location"]["uri"],
        part_uri
    );
    let changed = library.replace("(@bytes.slice $input 0 (@bytes.len $input))", "$input");
    let changed = session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":part_uri,"version":1,"text":changed}}}));
    assert!(
        changed
            .iter()
            .any(|notification| notification["params"]["uri"] == uri
                && notification["params"]["diagnostics"] == json!([])),
        "{changed:?}"
    );
}

#[test]
fn macro_generated_duplicates_share_one_warning_and_async_check_does_not_require_a_backend() {
    let expanded = COPY
        .replacen(
            "(module",
            r#"(module
      (@macro $copies (rule (?source:expr)
        (block (result $bytes) (drop (call $forward ?source)) (call $forward ?source))))"#,
            1,
        )
        .replace(
            "(call $forward $input)\n          (@recur",
            "(@$copies $input)\n          (@recur",
        );
    let warnings = report(&expanded);
    assert_eq!(
        warnings["performance"]["warnings"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "{warnings}"
    );
    let source = include_str!("component-fixtures/scoped-tasks.walz");
    assert_eq!(report(source)["performance"]["coverage"], "typed_core");
    let generic = COPY
        .replace("(func $copy (param", "(func $copy (@type.params $T) (param")
        .replace("(call $copy $input)", "(call (@inst $copy i32) $input)");
    assert_eq!(
        report(&generic)["performance"]["warnings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn documented_copy_rewrite_removes_the_warning_without_weakening_effect_checks() {
    let copied = report(include_str!("../examples/optimization/loop-copy.walz"));
    assert_eq!(
        copied["performance"]["warnings"].as_array().unwrap().len(),
        1
    );
    let indexed = report(include_str!("../examples/optimization/loop-indexed.walz"));
    assert_eq!(indexed["performance"]["warnings"], json!([]));
    let bad = HOST.replace(
        "(param $limit i32) (result i32)",
        "(param $limit i32) (result i32) (@effect pure)",
    );
    let output = check(&bad, &["--warn-performance", "--json"]);
    assert!(!output.status.success());
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        output["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("effect contract")
    );
    assert!(output.get("performance").is_none());
    let false_law = check(
        "(module (@law \"false\" (i32.const 0)))",
        &["--warn-performance", "--json"],
    );
    assert!(!false_law.status.success());
    let false_law: Value = serde_json::from_slice(&false_law.stdout).unwrap();
    assert_eq!(false_law["ok"], false);
    assert!(!false_law["diagnostics"].as_array().unwrap().is_empty());
    assert!(false_law.get("performance").is_none());
}

#[test]
#[ignore = "requires Binaryen 133; run just test-wasm-opt /path/to/wasm-opt"]
fn wasm_opt_advice_examples_preserve_values_traps_and_host_identity() {
    let optimizer = std::env::var_os("WALZ_WASM_OPT").expect("set WALZ_WASM_OPT to wasm-opt 133");
    let version = Command::new(&optimizer).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("version 133"));
    for (index, source) in [
        include_str!("../examples/optimization/loop-copy.walz"),
        include_str!("../examples/optimization/loop-indexed.walz"),
        HOST,
    ]
    .iter()
    .enumerate()
    {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wasm");
        let output = dir.path().join("out.wasm");
        std::fs::write(&input, walz::compile(source).unwrap().wasm).unwrap();
        let optimized = Command::new(&optimizer)
            .arg(&input)
            .args([
                "--enable-gc",
                "--enable-reference-types",
                "-O3",
                "--print",
                "-o",
            ])
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            optimized.status.success(),
            "{}",
            String::from_utf8_lossy(&optimized.stderr)
        );
        let wat = String::from_utf8(optimized.stdout).unwrap();
        if index == 0 {
            assert!(wat.contains("array.copy"));
        }
        if index == 1 {
            assert!(!wat.contains("array.copy"));
        }
        if index == 2 {
            assert!(wat.contains("struct.new"));
        }
        wasmparser::Validator::new()
            .validate_all(&std::fs::read(&output).unwrap())
            .unwrap();
        let js = if index < 2 {
            r#"import fs from 'node:fs';import assert from 'node:assert/strict';
            const {instance:{exports:e}}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]));
            assert.equal(e.main(),42);
            for(const length of [0,1,2,1024]){const input=e.make(length);assert.equal(e.sum(input,0),0);
              if(length===0)assert.throws(()=>e.sum(input,1),WebAssembly.RuntimeError);
              else for(const count of [1,2,17])assert.equal(e.sum(input,count),21*count);}"#
        } else {
            r#"import fs from 'node:fs';import assert from 'node:assert/strict';const refs=[];
            const {instance:{exports:e}}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{host:{save:value=>refs.push(value)}});
            assert.equal(e.main(17),17);assert.equal(refs.length,17);assert.equal(new Set(refs).size,17);"#
        };
        for path in [&input, &output] {
            let result = Command::new("node")
                .args(["--input-type=module", "-e", js])
                .arg(path)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            if index < 2 {
                let result = Command::new("wasmtime")
                    .args([
                        "run",
                        "-W",
                        "gc=y,function-references=y",
                        "--invoke",
                        "main",
                    ])
                    .arg(path)
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
            }
        }
    }
}

#[test]
fn lsp_performance_warnings_are_opt_in_cached_and_clear_on_edit_or_disable() {
    let dir = tempfile::tempdir().unwrap();
    let uri = walz::lsp::file_uri(&dir.path().join("main.walz")).unwrap();
    let text = COPY.replace("(call $forward $input)", "(; 🦀 ;) (call $forward $input)");
    let mut session = walz::lsp::Session::new(Default::default());
    session.handle(json!({"id":1,"method":"initialize","params":{}}));
    let opened = session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":text}}}));
    assert_eq!(opened[0]["params"]["diagnostics"], json!([]));
    let analyses = session.analysis_count();
    let settings = |enabled| json!({"method":"workspace/didChangeConfiguration","params":{"settings":{"walz":{"performanceWarnings":enabled}}}});
    let enabled = session.handle(settings(true));
    let warning = &enabled[0]["params"]["diagnostics"][0];
    assert_eq!(warning["code"], "W_OPT_LOOP_COPY", "{enabled:?}");
    assert_eq!(warning["severity"], 2);
    let offset = text.rfind("(call $forward $input)").unwrap();
    let column = text[..offset]
        .rsplit('\n')
        .next()
        .unwrap()
        .encode_utf16()
        .count();
    assert_eq!(warning["range"]["start"]["character"], column);
    assert_eq!(session.analysis_count(), analyses);
    let disabled = session.handle(settings(false));
    assert_eq!(disabled[0]["params"]["diagnostics"], json!([]));
    session.handle(settings(true));
    let invalid = session.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"(module (func $main (result i32) $missing))"}]}}));
    let diagnostics = invalid[0]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
}
