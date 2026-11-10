use std::{
    path::Path,
    process::{Command, Output},
};
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
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
fn review_output_is_reversible_and_review_files_build_and_run() {
    let root = tempfile::tempdir().unwrap();
    let source = r#"(module (func $main (export "main") (result i64) (@effect pure)
      (@let (($left (type i64) 40) ($right (type i64) 2)) (+ $left $right))))"#;
    std::fs::write(root.path().join("main.walz"), source).unwrap();
    let canonical = success(cli(root.path(), &["print", "main.walz"]));
    assert_eq!(canonical, walz::format::source(source).unwrap());
    let view = success(cli(
        root.path(),
        &["print", "--format", "watz", "main.walz"],
    ));
    assert!(view.contains("func $main(): i64"), "{view}");
    assert!(view.contains("@export(\"main\")"), "{view}");
    assert!(
        !view.contains("@wasmExport") && !view.contains("function "),
        "{view}"
    );
    assert!(
        view.contains("let $left: i64 = 40, $right: i64 = 2;"),
        "{view}"
    );
    std::fs::write(root.path().join("main.watz"), &view).unwrap();
    let lowered = success(cli(root.path(), &["print", "main.watz"]));
    assert_eq!(
        walz::compile(&lowered).unwrap().wasm,
        walz::compile(source).unwrap().wasm
    );
    success(cli(root.path(), &["check", "main.watz"]));
    success(cli(root.path(), &["build", "main.watz"]));
    assert!(root.path().join("target/main.wasm").is_file());
    assert_eq!(
        success(cli(root.path(), &["run", "main.watz"])).trim(),
        "42"
    );
    success(cli(root.path(), &["fmt", "main.watz"]));
    success(cli(root.path(), &["fmt-check", "main.watz"]));
    std::fs::write(root.path().join("math.test.watz"), &view).unwrap();
    success(cli(root.path(), &["build", "math.test.watz"]));
    assert!(root.path().join("target/math.test.wasm").is_file());
    std::fs::remove_file(root.path().join("main.walz")).unwrap();
    success(cli(root.path(), &["build"]));
    assert_eq!(success(cli(root.path(), &["run"])).trim(), "42");
}

#[test]
fn print_formats_are_explicit_read_only_and_validate_options() {
    let root = tempfile::tempdir().unwrap();
    let source = "(module (func $f (result i32) (i64.const 1)))";
    std::fs::write(root.path().join("source.walz"), source).unwrap();
    assert!(success(cli(root.path(), &["print", "--help"])).contains("--format"));
    assert_eq!(
        success(cli(root.path(), &["print", "source.walz"])),
        success(cli(
            root.path(),
            &["print", "source.walz", "--format", "walz"]
        ))
    );
    assert!(
        success(cli(root.path(), &["print", "source.walz", "--format=watz"]))
            .contains("func $f(): i32")
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("source.walz")).unwrap(),
        source
    );
    assert!(!cli(root.path(), &["check", "source.walz"]).status.success());
    for args in [
        vec!["print"],
        vec!["print", "--format"],
        vec!["print", "--format", "typescript", "source.walz"],
        vec!["print", "--format=", "source.walz"],
        vec![
            "print",
            "--format",
            "watz",
            "--format",
            "walz",
            "source.walz",
        ],
        vec!["print", "source.walz", "other.walz"],
        vec!["print", "--unknown", "source.walz"],
    ] {
        let out = cli(root.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
    std::fs::write(root.path().join("-source.walz"), source).unwrap();
    assert_eq!(
        success(cli(root.path(), &["print", "--", "-source.walz"])),
        walz::format::source(source).unwrap()
    );
}

#[test]
fn source_publication_and_inline_wasm_exports_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    let source = "(module (@export (func $answer)) (func $answer (export \"wasm-answer\") (result i32) (i32.const 42)))";
    std::fs::write(root.path().join("main.walz"), source).unwrap();
    let view = success(cli(
        root.path(),
        &["print", "main.walz", "--format", "watz"],
    ));
    assert!(view.contains("export { $answer };"), "{view}");
    assert!(view.contains("@export(\"wasm-answer\")"), "{view}");
    std::fs::write(root.path().join("main.watz"), view).unwrap();
    let loaded = walz::source::load(root.path().join("main.walz")).unwrap();
    let restored = success(cli(root.path(), &["print", "main.watz"]));
    let lowered = root.path().join("restored.walz");
    std::fs::write(&lowered, restored).unwrap();
    let restored = walz::source::load(lowered).unwrap();
    assert_eq!(
        walz::compile(loaded.text()).unwrap().wasm,
        walz::compile(restored.text()).unwrap().wasm
    );
    assert_eq!(
        success(cli(
            root.path(),
            &["run", "main.watz", "--invoke", "wasm-answer"]
        ))
        .trim(),
        "42"
    );
}

#[test]
fn review_modules_mix_with_sexpressions_and_keep_diagnostic_locations() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("math.watz"),
        "export { $add }; func $add($a: i64, $b: i64): i64 { return ($a + $b); }",
    )
    .unwrap();
    std::fs::write(root.path().join("main.walz"), "(module (@use \"./math.watz\" (func $add)) (func $main (export \"main\") (result i64) (call $add 40 2)))").unwrap();
    assert_eq!(
        success(cli(root.path(), &["run", "main.walz"])).trim(),
        "42"
    );
    std::fs::write(root.path().join("helper.walz"), "(module (@export (func $add)) (func $add (param $a i64) (param $b i64) (result i64) (+ $a $b)))").unwrap();
    std::fs::write(root.path().join("main.watz"), "module { import { $add } from \"./helper.walz\"; @export(\"main\") func $main(): i64 { return $add(40, 2); } }").unwrap();
    assert_eq!(
        success(cli(root.path(), &["run", "main.watz"])).trim(),
        "42"
    );
    let bad = "// 日本語 😀\nfunc $bad(): i32 {\n    return wat.i64.const(1);\n}\n";
    std::fs::write(root.path().join("bad.watz"), bad).unwrap();
    let output = cli(root.path(), &["check", "--json", "bad.watz"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let location = &report["diagnostics"][0]["primary"]["location"];
    assert_eq!(location["line"], 3);
    assert_eq!(location["column"], 12);
    assert!(location["path"].as_str().unwrap().ends_with("bad.watz"));
    std::fs::write(
        root.path().join("math.watz"),
        "module { export { $add }; func $add($a: i64, $b: i64): i64 { return wat.i32.const(0); } }",
    )
    .unwrap();
    let output = cli(root.path(), &["check", "--json", "main.walz"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["diagnostics"][0]["primary"]["location"]["path"]
            .as_str()
            .unwrap()
            .ends_with("math.watz")
    );
}

#[test]
fn review_type_queries_map_instruction_names_back_to_the_input() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("query.watz"),
        "module {\n func $f(): i64 {\n  return wat.i64.const(42);\n }\n}",
    )
    .unwrap();
    let output = success(cli(root.path(), &["type-at", "query.watz", "3", "10"]));
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["matches"][0]["ty"], "i64");
}

#[test]
fn direct_controls_run_once_preserve_branches_and_report_original_positions() {
    let root = tempfile::tempdir().unwrap();
    let source = include_str!("component-fixtures/match-dispatch.walz");
    std::fs::write(root.path().join("main.walz"), source).unwrap();
    let view = success(cli(
        root.path(),
        &["print", "--format", "watz", "main.walz"],
    ));
    assert!(view.contains("match ("), "{view}");
    assert!(view.contains("else if<ref<$Event>>"), "{view}");
    let lowered = walz::review::from_watz(&view).unwrap();
    for optimize in [false, true] {
        let options = walz::CompileOptions { optimize };
        assert_eq!(
            walz::compile_with_options(source, options).unwrap().wasm,
            walz::compile_with_options(&lowered.text, options)
                .unwrap()
                .wasm
        );
    }
    std::fs::write(root.path().join("main.watz"), view).unwrap();
    for (args, expected) in [
        (vec!["run", "main.watz"], "42"),
        (
            vec!["run", "main.watz", "--invoke", "run", "--", "7", "35"],
            "42",
        ),
        (
            vec!["run", "main.watz", "--invoke", "fallback", "--", "1"],
            "42",
        ),
        (
            vec!["run", "main.watz", "--invoke", "fallback", "--", "0"],
            "7",
        ),
    ] {
        assert_eq!(success(cli(root.path(), &args)).trim(), expected);
    }
    success(cli(root.path(), &["fmt", "main.watz"]));
    success(cli(root.path(), &["fmt-check", "main.watz"]));

    let counter = r#"(module
      (type $Counter (struct (field $n (mut i32))))
      (func $main (export "main") (result i32) (@effect reads writes allocates)
        (@let $c (struct.new $Counter 0)
          (+ (if (result i32)
               (block (result i32)
                 (struct.set $Counter $n $c (+ (struct.get $Counter $n $c) 1)) 1)
               (then 40)
               (else (block (result i32) (struct.set $Counter $n $c 99) 0)))
             (* (struct.get $Counter $n $c) 2)))))"#;
    std::fs::write(root.path().join("counter.walz"), counter).unwrap();
    let view = success(cli(
        root.path(),
        &["print", "counter.walz", "--format", "watz"],
    ));
    assert!(view.contains("if<i32> ("), "{view}");
    std::fs::write(root.path().join("counter.watz"), view).unwrap();
    assert_eq!(
        success(cli(root.path(), &["run", "counter.watz"])).trim(),
        "42"
    );

    let bad = "// 日本語 😀\nfunc $bad(): i32 {\n  return if<i32> (1) {\n    wat.i64.const(1)\n  } else { 0 };\n}\n";
    std::fs::write(root.path().join("bad.watz"), bad).unwrap();
    let output = cli(root.path(), &["check", "--json", "bad.watz"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["diagnostics"][0]["code"], "E_TYPE_MISMATCH");
    assert_eq!(report["diagnostics"][0]["primary"]["location"]["line"], 4);
    assert_eq!(report["diagnostics"][0]["primary"]["location"]["column"], 5);
}

#[test]
fn review_tests_and_benchmarks_use_the_ordinary_runners() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("tests")).unwrap();
    std::fs::write(root.path().join("tests/value.test.watz"), "module { walz.test(\"sum\", (wat.i32.const(40) === (wat.i32.const(42) - wat.i32.const(2)))); }").unwrap();
    assert!(success(cli(root.path(), &["test"])).contains("1 passed; 0 failed; 0 errors"));
    std::fs::write(
        root.path().join("value_bench.watz"),
        "module { walz.bench(\"sum\", wat.result(i64), wat.i64.const(42)); }",
    )
    .unwrap();
    assert!(success(cli(root.path(), &["bench", "--list"])).contains("sum"));
}

#[test]
fn review_examples_and_semantic_rejections_use_the_same_backend() {
    let canonical = include_str!("../examples/review/main.walz");
    let view = include_str!("../examples/review/main.watz");
    assert_eq!(walz::review::to_watz(canonical).unwrap(), view);
    let restored = walz::review::from_watz(view).unwrap();
    assert_eq!(
        walz::compile(canonical).unwrap().wasm,
        walz::compile(&restored.text).unwrap().wasm
    );
    let guide = include_str!("../docs/review-syntax.md");
    let documented = guide
        .split("```watz\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    assert_eq!(
        walz::compile(canonical).unwrap().wasm,
        walz::compile(&walz::review::from_watz(documented).unwrap().text)
            .unwrap()
            .wasm
    );
    for bad in [
        "(module (func $f (result i32) (i64.const 1)))",
        "(module (import \"host\" \"work\" (func $work (result i32))) (func $f (result i32) (@effect pure) (call $work)))",
        "(module (@use \"walz:core\" (type $box)) (func $f (@let $owner (@box.new i32 1) (nop))))",
        "(module (func $f (result i32) (if (result i32) 1 (then (i64.const 1)) (else 0))))",
        "(module (import \"host\" \"work\" (func $work (result i32))) (func $f (result i32) (@effect pure) (if (result i32) 0 (then (call $work)) (else 0))))",
        "(module (@use \"walz:core\" (type $box)) (func $f (@let $owner (@box.new i32 1) (if 1 (then (@box.close (@move $owner))) (else (nop))))))",
        "(module (@use \"walz:core\" (type $result)) (func $f (param $v (@inst $result i32 i32)) (result i32) (@match $v (@case ok ($n) $n) (@default $n))))",
        "(module (@use \"walz:core\" (type $result) (type $box)) (func $f (param $v (@inst $result (@inst $box i32) i32)) (result i32) (@match (@move $v) (@default 0))))",
    ] {
        let expected = walz::compile(bad).err().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("bad.watz"),
            walz::review::to_watz(bad).unwrap(),
        )
        .unwrap();
        let output = cli(root.path(), &["check", "--json", "bad.watz"]);
        assert_eq!(output.status.code(), Some(1));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["diagnostics"][0]["code"], expected.diagnostic().code);
    }
}
