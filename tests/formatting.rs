use std::process::Command;

#[test]
fn formatter_preserves_native_tokens_comments_and_is_idempotent() {
    for source in [
        "(module ;; keep 日本語\n(memory 1)(data(i32.const 0)\"\\00\\ff(;not a comment;)\") (func(export \"main\")(result i32) i32.const 42))",
        "(module (;outer (;nested;) ;) (func $main (export \"main\") (result %text) (%text.lit \"日本語🦀\\n\")))",
    ] {
        let formatted = wlt::format::source(source).unwrap();
        assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
        assert!(formatted.contains(if source.contains(";;") {
            ";; keep 日本語"
        } else {
            "(;outer (;nested;) ;)"
        }));
        assert_eq!(
            wlt::compile(source).unwrap().wasm,
            wlt::compile(&formatted).unwrap().wasm
        );
    }
}

#[test]
fn format_cli_checks_then_writes_only_a_valid_source_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.wlt");
    std::fs::write(&path, "(module(func $f (nop)))").unwrap();
    let run = |command| {
        Command::new(env!("CARGO_BIN_EXE_wlt"))
            .arg(command)
            .arg(&path)
            .output()
            .unwrap()
    };
    assert!(!run("fmt-check").status.success());
    assert!(run("fmt").status.success());
    assert!(run("fmt-check").status.success());
    let broken = "(module\n(func $f (nop))";
    std::fs::write(&path, broken).unwrap();
    let output = run("fmt");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("1:1"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), broken);
}

#[test]
fn formatter_groups_contracts_and_macro_headers_and_attaches_closing_delimiters() {
    let source = r#"(module
      (func $convert (param $input (%result i64 i32))
        (result (%result i32 i32)) (%effect pure)
        (%result.let ($value) (%result i64 i32) i32 (local.get $input)
          (%result.ok i32 i32 (i32.wrap_i64 (local.get $value))))))"#;
    let formatted = wlt::format::source(source).unwrap();
    assert!(formatted.contains("(func $convert\n    (param $input (%result i64 i32))"));
    assert!(formatted.contains("(%result.let ($value) (%result i64 i32) i32\n"));
    assert!(formatted.lines().filter(|line| line.trim() == ")").count() <= 1);
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn formatter_keeps_fold_types_and_case_bindings_together() {
    let source = include_str!("../docs/readability/before/counted.gwat");
    let formatted = wlt::format::source(source).unwrap();
    assert!(formatted.contains("(stream.fold-result text (pair (ref $stock.State) i32) i32\n"));
    assert!(formatted.contains("(case pair ($ledger $count)\n"));
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
}

#[test]
fn formatter_does_not_put_a_closing_delimiter_in_a_line_comment() {
    let source =
        "(module (func $f (result i32) (block (result i32) (i32.const 42) ;; final expression\n)))";
    let formatted = wlt::format::source(source).unwrap();
    assert!(formatted.contains(";; final expression\n"));
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
}

#[test]
fn formatter_preserves_catalog_and_application_binaries() {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../examples/catalog.json")).unwrap();
    let paths = catalog["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap());
    for path in paths.chain([
        "examples/apps/stock-ledger/main.wlt",
        "examples/apps/log-stats/main.wlt",
    ]) {
        let loaded = wlt::source::load(path).unwrap();
        let formatted = wlt::format::source(loaded.text()).unwrap();
        assert_eq!(
            wlt::format::source(&formatted).unwrap(),
            formatted,
            "{path}"
        );
        assert_eq!(
            wlt::compile(loaded.text()).unwrap().wasm,
            wlt::compile(&formatted).unwrap().wasm,
            "{path}"
        );
    }
}

#[test]
fn formatter_bounds_closing_groups_in_deeply_nested_code() {
    let source = "(module (func $f (result i32) (%let $a (i32.const 42) (%let $b (local.get $a) (%let $c (local.get $b) (i32.add (local.get $c) (i32.const 0)))))))";
    let formatted = wlt::format::source(source).unwrap();
    assert!(
        formatted
            .lines()
            .all(|line| line.chars().rev().take_while(|c| *c == ')').count() <= 4),
        "{formatted}"
    );
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn formatter_keeps_short_macro_inputs_out_of_the_header() {
    let source = r#"(module (func $main (result (%result i64 i32))
      (%result.let ($total)
        (%stream.fold-result (%stream.empty (%result i64 i32)) (i64.const 0) ($sum $item)
          (%result.ok (i64.add (local.get $sum) (local.get $item))))
        (%result.ok (local.get $total)))))"#;
    let formatted = wlt::format::source(source).unwrap();
    assert!(formatted.contains("(%result.let ($total)\n"), "{formatted}");
    assert!(formatted.contains("(%stream.fold-result\n"), "{formatted}");
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
}

#[test]
fn formatter_exposes_pattern_let_continuation_even_when_it_fits_one_line() {
    let source = "(module (func $f (param $p (%pair i32 i32)) (result i32) (%let ($a $b) (local.get $p) (local.get $b))))";
    let formatted = wlt::format::source(source).unwrap();
    assert!(
        formatted.contains("(%let ($a $b) (local.get $p)\n"),
        "{formatted}"
    );
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn grouped_let_keeps_the_continuation_out_of_the_binding_header() {
    let source = "(module (func $main (export \"main\") (result i64) (%let (($a (i64.const 40)) ($b (i64.const 42))) (local.get $b))))";
    let formatted = wlt::format::source(source).unwrap();
    assert!(
        formatted
            .lines()
            .any(|line| line.trim_start().starts_with("(local.get $b)")),
        "{formatted}"
    );
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn grouped_bindings_align_and_long_result_subjects_separate_the_continuation() {
    let source = r#"(module
      (func $main (result (%result i64 i32)) (%effect suspends)
        (%let (($base (i64.const 40)) ($initial (i64.add (local.get $base) (i64.const 2))))
          (%result.let ($total)
            (%stream.fold-result (%stream.empty (%result i64 i32)) (local.get $initial) ($sum $item)
              (%result.ok (i64.add (local.get $sum) (local.get $item))))
            (%result.ok (local.get $total))))))"#;
    let formatted = wlt::format::source(source).unwrap();
    assert!(
        formatted.contains("(%let\n      (($base (i64.const 40))\n       ($initial"),
        "{formatted}"
    );
    assert!(
        formatted.contains("\n\n        (%result.ok (local.get $total))"),
        "{formatted}"
    );
    assert!(
        formatted.contains("(func $main\n    (result (%result i64 i32))\n    (%effect suspends)"),
        "{formatted}"
    );
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn grouped_binding_comments_keep_their_order_and_do_not_swallow_delimiters() {
    let source = "(module (func $f (result i64) (%let (;first;) (($a (i64.const 40)) ;; between\n ($b (i64.add (local.get $a) (i64.const 2))) ;; final\n) (local.get $b))))";
    let formatted = wlt::format::source(source).unwrap();
    assert!(formatted.contains("(%let\n"), "{formatted}");
    assert!(formatted.contains(";; final\n"), "{formatted}");
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        wlt::compile(source).unwrap().wasm,
        wlt::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn formatting_an_empty_binding_list_preserves_its_delimiters() {
    // Formatting preserves tokens even when the compiler would reject a form.
    let formatted = wlt::format::source("(module (%let () (nop)))").unwrap();
    assert!(formatted.contains("()"), "{formatted}");
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
}
