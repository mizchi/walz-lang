use std::process::Command;

#[test]
fn standard_library_has_executable_language_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "tests/library.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("3 passed; 0 failed; 0 errors"));
}

#[test]
fn standard_library_search_parse_and_builder_contracts_execute_natively() {
    let output = Command::new(env!("CARGO_BIN_EXE_wlt"))
        .args(["test", "tests/stdlib.test.wlt"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("9 passed; 0 failed; 0 errors"));
}

#[test]
fn builder_representation_and_mutation_effects_are_checked() {
    let loaded = wlt::source::load("tests/stdlib.test.wlt").unwrap();
    let prefix = loaded.text().trim_end().strip_suffix(')').unwrap();
    for (body, message) in [
        (
            "(func $bad (param $b (ref $string-builder)) (%effect pure) (call $string-builder.text (local.get $b) (%string.lit \"x\")))",
            "effect contract",
        ),
        (
            "(func $bad (param $b (ref $string-builder)) (result i32) (struct.get $string-builder $length (local.get $b)))",
            "opaque",
        ),
        (
            "(func $bad (param $o (ref $json-object)) (%effect pure) (call $json.object-builder.u64 (local.get $o) (%string.lit \"n\") (i64.const 1)))",
            "effect contract",
        ),
    ] {
        assert!(
            wlt::check(&format!("{prefix} {body})"))
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
    let module = wlt::check(loaded.text()).unwrap();
    let analysis = wlt::analysis::analyze(&module);
    for name in [
        "$bytes.find-byte",
        "$bytes.find",
        "$bytes.parse-u64",
        "$string.parse-s64",
    ] {
        let effects = &analysis
            .functions
            .iter()
            .find(|f| f.name.ends_with(name.trim_start_matches('$')))
            .unwrap()
            .semantic_effects;
        assert!(!effects.reads && !effects.writes && !effects.host && !effects.suspends);
    }
}

#[test]
fn array_to_bytes_copies_and_checks_unsigned_octets() {
    let source = r#"(module
      (type $A (array (mut i32)))
      (func $main (export "main") (result i32)
        (%let $a (array.new_fixed $A 2 (i32.const 65) (i32.const 255))
          (%let $b (%bytes.from-array $A (local.get $a))
            (block (result i32)
              (array.set $A (local.get $a) (i32.const 0) (i32.const 0))
              (i32.add (%bytes.get (local.get $b) (i32.const 0)) (%bytes.get (local.get $b) (i32.const 1)))))))
      (func $bad (export "bad") (result %bytes)
        (%bytes.from-array $A (array.new_fixed $A 1 (i32.const 256)))))"#;
    let compiled = wlt::compile(source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("program.wasm");
    std::fs::write(&path, compiled.wasm).unwrap();
    for (export, success) in [("main", true), ("bad", false)] {
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y",
                "--invoke",
                export,
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if success {
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "320");
        }
    }
    for (element, effect, ok) in [
        ("(mut i32)", "pure", false),
        ("i32", "pure", true),
        ("i64", "reads", false),
    ] {
        let source = format!(
            "(module (type $A (array {element})) (func $f (param $a (ref $A)) (result %bytes) (%effect {effect}) (%bytes.from-array $A (local.get $a))))"
        );
        assert_eq!(wlt::check(&source).is_ok(), ok, "{source}");
    }
    let source = "(module (type $A (array (mut i32))) (func $copy (param $a (ref $A)) (result %bytes) (%bytes.from-array $A (local.get $a))))";
    let module = wlt::check(source).unwrap();
    let analysis = wlt::analysis::analyze(&module);
    let memory = &analysis.functions[0].memory;
    assert!(
        memory
            .reads
            .iter()
            .any(|l| matches!(l.object.root, wlt::regions::Root::Parameter { index: 0 }))
    );
    assert!(
        memory
            .returns
            .iter()
            .all(|p| matches!(p.root, wlt::regions::Root::Allocation { .. }))
    );
    wlt::compile("(module (type $A (array i32)) (func $f (result %bytes) (%bytes.from-array $A (unreachable))))").unwrap();
}
