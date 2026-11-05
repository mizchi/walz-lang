use std::process::Command;

#[test]
fn standard_library_has_executable_language_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_gcwat"))
        .args(["test", "tests/library.test.gwat"])
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
fn array_to_bytes_copies_and_checks_unsigned_octets() {
    let source = r#"(module
      (type $A (array (mut i32)))
      (func $main (export "main") (result i32)
        (let $a (array.new_fixed $A 2 (i32.const 65) (i32.const 255))
          (let $b (bytes.from-array $A (local.get $a))
            (block (result i32)
              (array.set $A (local.get $a) (i32.const 0) (i32.const 0))
              (i32.add (bytes.get (local.get $b) (i32.const 0)) (bytes.get (local.get $b) (i32.const 1)))))))
      (func $bad (export "bad") (result bytes)
        (bytes.from-array $A (array.new_fixed $A 1 (i32.const 256)))))"#;
    let compiled = gcwat::compile(source).unwrap();
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
            "(module (type $A (array {element})) (func $f (param $a (ref $A)) (result bytes) (effects {effect}) (bytes.from-array $A (local.get $a))))"
        );
        assert_eq!(gcwat::check(&source).is_ok(), ok, "{source}");
    }
    let source = "(module (type $A (array (mut i32))) (func $copy (param $a (ref $A)) (result bytes) (bytes.from-array $A (local.get $a))))";
    let module = gcwat::check(source).unwrap();
    let analysis = gcwat::analysis::analyze(&module);
    let memory = &analysis.functions[0].memory;
    assert!(
        memory
            .reads
            .iter()
            .any(|l| matches!(l.object.root, gcwat::regions::Root::Parameter { index: 0 }))
    );
    assert!(
        memory
            .returns
            .iter()
            .all(|p| matches!(p.root, gcwat::regions::Root::Allocation { .. }))
    );
    gcwat::compile("(module (type $A (array i32)) (func $f (result bytes) (bytes.from-array $A (unreachable))))").unwrap();
}
