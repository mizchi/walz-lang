use serde_json::{Value, json};
use std::process::Command;

#[test]
fn core_js_exposes_checked_esm_names_shapes_and_suspension() {
    let source = r#"(module
      (%use "wlt:core" (type $string) (type $bytes) (type $future))
      (func $add (export "add-one") (param $x i64) (result i64) (%effect pure) (+ $x 1))
      (func $echo (export "echo") (param $x $string) (result $string) (%effect pure) $x)
      (func $bytes (export "bytes") (param $x $bytes) (result $bytes) (%effect pure) $x)
      (func $wait (export "wait") (param $x (%inst $future i32)) (result i32) (%effect suspends)
        (%future.await i32 (%move $x))))"#;
    let built = wlt::component::core_js::compile(source).unwrap();
    let esm = &built.manifest["esm"];
    assert_eq!(esm["schema"], "wlt-esm-v1");
    assert_eq!(
        esm["exports"]["add-one"],
        json!({"jsName":"addOne","params":["i64"],"result":"i64","suspends":false})
    );
    assert_eq!(esm["exports"]["echo"]["result"], "text");
    assert_eq!(esm["exports"]["bytes"]["params"], json!(["bytes"]));
    assert_eq!(esm["exports"]["wait"]["params"], json!(["(future i32)"]));
    assert_eq!(esm["exports"]["wait"]["suspends"], true);
}

#[test]
fn core_js_build_records_files_for_incremental_consumers() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    let helper = dir.path().join("helper.wlt");
    let policy = dir.path().join("bindings.json");
    let output = dir.path().join("output");
    std::fs::write(
        &main,
        r#"(module (%use "./helper.wlt" (func $answer))
      (func $main (export "main") (result i32) (%effect pure) (call $answer)))"#,
    )
    .unwrap();
    std::fs::write(
        &helper,
        r#"(module (%export (func $answer))
      (func $answer (result i32) (%effect pure) 42))"#,
    )
    .unwrap();
    std::fs::write(&policy, r#"{"imports":[]}"#).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .arg("build")
        .arg(&main)
        .args(["--target", "core-js", "--bindings"])
        .arg(&policy)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let metadata: Value =
        serde_json::from_slice(&std::fs::read(output.join("manifest.json")).unwrap()).unwrap();
    let files = metadata["sourceFiles"].as_array().unwrap();
    for path in [&main, &helper, &policy] {
        assert!(
            files.contains(&json!(path.canonicalize().unwrap())),
            "{metadata}"
        );
    }
    assert_eq!(metadata["esm"]["exports"]["main"]["result"], "i32");
}
