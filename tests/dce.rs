#[test]
fn checked_output_retains_only_exported_functions_and_transitive_dependencies() {
    let source = r#"(module
      (%namespace $lib
        (%public (func $answer) (func $unused))
        (import "wlt:host/event-loop@0.1.0" "defer-i64"
          (func $sleep (param $n i32) (param $value i64) (result (%future i64))))
        (type $Unused (struct (field i64)))
        (func $unused (result (%future i64)) (call $sleep (i32.const 1) (i64.const 1)))
        (func $helper (result i32) (i32.const 42))
        (func $answer (result i32) (call $helper)))
      (func $main (export "main") (result i32) (call $lib.answer)))"#;
    let compiled = wlt::compile(source).unwrap();
    assert!(compiled.wat.contains("(func $lib.answer"));
    assert!(compiled.wat.contains("(func $lib.helper"));
    for removed in [
        "$lib.unused",
        "$lib.sleep",
        "$lib.Unused",
        "wlt:host/event-loop",
        "__delivery",
    ] {
        assert!(
            !compiled.wat.contains(removed),
            "{removed}: {}",
            compiled.wat
        );
    }
    // Inspection still sees the complete checked source, including dead code.
    assert!(
        compiled
            .ir
            .unwrap()
            .functions
            .iter()
            .any(|f| f.name == "$lib.unused")
    );
}

#[test]
fn unused_trusted_imports_do_not_require_host_capabilities_or_abi_codecs() {
    let source = r#"(module
      (import "wlt:host/event-loop@0.1.0" "defer-i64"
        (func $sleep (param $n i32) (param $value i64) (result (%future i64))))
      (func $main (export "main") (result i32) (%effect pure) (i32.const 42)))"#;
    let core = wlt::component::core_js::compile(source).unwrap();
    let component = wlt::component::compile(source).unwrap();
    assert_eq!(core.manifest["imports"].as_array().unwrap().len(), 0);
    assert_eq!(component.manifest["imports"].as_array().unwrap().len(), 0);
    assert!(!core.bindings.contains("defer-i64"));
    assert!(!component.wat.contains("wlt:host/event-loop"));
}

#[test]
fn unreachable_code_is_still_type_effect_and_ownership_checked() {
    for body in [
        "(func $bad (result i32) (i64.const 1))",
        "(func $bad (param $b (ref $B)) (%effect pure) (struct.set $B 0 (local.get $b) (i32.const 1)))",
        "(func $bad (param $s (%stream i32)) (nop))",
    ] {
        let source = format!(
            "(module (type $B (struct (field (mut i32)))) {body} (func $main (export \"main\") (result i32) (i32.const 42)))"
        );
        assert!(wlt::compile(&source).is_err(), "{source}");
    }
}

#[test]
fn native_wat_with_initialization_or_unused_imports_is_preserved() {
    let source = r#"(module (import "host" "unused" (func)) (func $init) (start $init))"#;
    assert_eq!(wlt::compile(source).unwrap().wat, source);
}

#[test]
fn projection_keeps_closure_environments_recursive_types_tags_and_tests() {
    let source = r#"(module
      (type $Dead (array i32))
      (tag $DeadTag (param i32))
      (type $Cell (struct (field $n i32) (field $next (ref null $Cell))))
      (tag $Error (param i32))
      (%closure $Read (%capture $cell (ref $Cell)) (param $n i32) (result i32)
        (i32.add (struct.get $Cell $n (local.get $cell)) (local.get $n)))
      (func $dead (result i32) (i32.const 0))
      (func $main (export "main") (result i32)
        (%let $read (%closure.new $Read (struct.new $Cell (i32.const 40) (ref.null $Cell)))
          (%try (result i32) (do (throw $Error (i32.const 2)))
            (catch $Error ($n) (%closure.call $Read (local.get $read) (local.get $n))))))
      (%test "reachable test" (i32.eq (call $main) (i32.const 42))))"#;
    let compiled = wlt::compile(source).unwrap();
    assert!(!compiled.wat.contains("$Dead"));
    assert!(!compiled.wat.contains("(func $dead"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("closure.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    for (export, expected) in [
        ("main", "42"),
        (compiled.ir.as_ref().unwrap().tests[0].export.as_str(), "1"),
    ] {
        let out = std::process::Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                export,
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), expected);
    }
}
