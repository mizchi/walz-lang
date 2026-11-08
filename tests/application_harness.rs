const SOURCE: &str = r#"(module
  (type $Private (struct (field i32)))
  (func $main (export "main") (result i32) (%effect pure) 42)
  (%test "not executed by build" 0)
  (%bench "private value" (result (ref $Private)) (struct.new $Private 42)))"#;

#[test]
fn application_backends_hide_harness_exports_after_checking_the_whole_source() {
    let core = wlt::compile(SOURCE).unwrap();
    // Core retains the low-level harness ABI used by native tooling.
    assert_eq!(core.ir.unwrap().benches.len(), 1);
    let js = wlt::component::core_js::compile(SOURCE).unwrap();
    assert_eq!(
        js.exports.keys().map(String::as_str).collect::<Vec<_>>(),
        ["main"]
    );
    let component = wlt::component::compile(SOURCE).unwrap();
    assert!(!component.wat.contains("__wlt_bench_"));
    let browser = wlt::component::core_js::compile_for_browser(SOURCE, Default::default()).unwrap();
    assert_eq!(browser.exports.len(), 1);
    for source in [
        SOURCE.replace(
            "struct.new $Private 42",
            "struct.new $Private (i64.const 42)",
        ),
        SOURCE.replace(
            "(%test \"not executed by build\" 0)",
            "(%test \"invalid\" (i64.const 1))",
        ),
    ] {
        assert!(wlt::compile(&source).is_err());
        assert!(wlt::component::compile(&source).is_err());
        assert!(wlt::component::core_js::compile(&source).is_err());
    }
}

#[test]
fn application_selection_uses_declarations_not_a_reserved_export_name_pattern() {
    let source =
        "(module (func $user (export \"__wlt_bench_0\") (nop)) (%bench \"generated\" (nop)))";
    let compiled = wlt::compile(source).unwrap();
    assert!(compiled.wat.contains("(export \"__wlt_bench_0\")"));
    // A user-written invalid WIT name remains an error at that boundary.
    assert!(wlt::component::compile(source).is_err());
}
