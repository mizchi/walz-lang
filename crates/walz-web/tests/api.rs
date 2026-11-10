use serde_json::{Value, json};

const EXTENSION: &str = r#"(module
  (@use "walz:core" (type $result))
  (func $main (export "main") (result i64) (@effect pure)
    (@match (@inst $result i64 i32) (result i64) (@result.ok i64 i32 (i64.const 42))
      (@case ok ($value) $value)
      (@case err ($error) (i64.const 0)))))"#;

#[test]
fn bridge_preserves_native_and_extension_compilation() {
    for source in [
        r#"(module (func (export "main") (result i32) i32.const 42))"#,
        EXTENSION,
    ] {
        for optimize in [true, false] {
            let expected =
                walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
            let actual: Value =
                serde_json::from_str(&walz_web::compile_source(source, optimize)).unwrap();
            assert_eq!(actual["schema"], "walz-playground-v1");
            assert_eq!(actual["status"], "success");
            assert_eq!(actual["wat"], expected.wat);
            assert_eq!(actual["wasm"], json!(expected.wasm));
            assert_eq!(actual["analysis"], json!(expected.analysis));
        }
    }
}

#[test]
fn bridge_preserves_diagnostics_and_recovers_after_failure() {
    let source = ";; 日本語🦀\n(module (func $main (result i64) (i32.const 1)))";
    let expected = walz::compile(source).err().unwrap();
    let actual: Value = serde_json::from_str(&walz_web::compile_source(source, true)).unwrap();
    assert_eq!(actual["status"], "error");
    assert_eq!(actual["diagnostic"], json!(expected.diagnostic()));
    assert!(actual.get("wasm").is_none());
    let good: Value = serde_json::from_str(&walz_web::compile_source("(module)", true)).unwrap();
    assert_eq!(good["status"], "success");
}

const ASYNC: &str = r#"(module
  (@use "walz:core" (type $future))
  (import "walz:host/event-loop@0.1.0" "defer-i64"
    (func $defer (param $ms i32) (param $value i64) (result (@inst $future i64))))
  (func $main (export "read-value") (result i64) (@effect host suspends)
    (@future.await i64 (call $defer 1 42)))
  (func $identity (export "needs-value") (param $n i64) (result i64) (@effect pure) $n))"#;

#[test]
fn browser_execution_matches_native_codegen_and_preserves_export_metadata() {
    let expected = walz::component::core_js::compile(ASYNC).unwrap();
    let actual: Value = serde_json::from_str(&walz_web::prepare_execution(ASYNC, true)).unwrap();
    assert_eq!(actual["schema"], "walz-playground-execution-v1");
    assert_eq!(actual["status"], "success");
    assert_eq!(actual["program"], json!(expected));
    assert_eq!(
        actual["program"]["exports"]["read-value"],
        json!({"js_name":"readValue","arity":0})
    );
    assert_eq!(
        actual["program"]["exports"]["needs-value"],
        json!({"js_name":"needsValue","arity":1})
    );
}

#[test]
fn browser_execution_keeps_type_effect_ownership_binding_and_proof_checks() {
    for (source, code) in [
        (ASYNC.replace("(call $defer 1 42)", "(call $defer 1 (i32.const 42))"), "E_TYPE_MISMATCH"),
        (ASYNC.replace("(@effect host suspends)", "(@effect pure)"), "E_EFFECT_CONTRACT"),
        (ASYNC.replace("walz:host/event-loop@0.1.0", "untrusted"), "E_CHECK"),
        ("(module (import \"untrusted\" \"read\" (func $read (result i32))) (func $main (export \"main\") (result i32) (call $read)))".into(), "E_TOOL"),
        ("(module (@law \"true\" (i32.const 1)) (func $main (export \"main\") (result i32) (@effect pure) 1))".into(), "E_HOST_UNAVAILABLE"),
        ("(module (@use \"walz:core\" (type $box)) (func $main (export \"main\") (@effect pure) (@let $x (@box.new i64 1) (block (@box.close (@move $x)) (@box.close (@move $x))))))".into(), "E_OWNERSHIP_MOVED"),
    ] {
        let actual: Value = serde_json::from_str(&walz_web::prepare_execution(&source, true)).unwrap();
        assert_eq!(actual["status"], "error", "{source}: {actual}");
        assert_eq!(actual["diagnostic"]["code"], code, "{source}: {actual}");
        assert!(actual.get("program").is_none());
    }
}

#[test]
fn scoped_tasks_compile_to_a_browser_execution_bundle() {
    let source = include_str!("../../../tests/component-fixtures/scoped-tasks.walz");
    for optimize in [true, false] {
        let actual: Value =
            serde_json::from_str(&walz_web::compile_source(source, optimize)).unwrap();
        assert_eq!(actual["status"], "success", "{actual}");
        assert_eq!(actual["execution"]["manifest"]["childTasks"], true);
        assert_eq!(actual["wasm"], actual["execution"]["wasm"]);
        assert!(actual["wat"].as_str().unwrap().contains("__child_run"));
    }
}
