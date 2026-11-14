use std::process::Command;

#[test]
fn compiler_parts_are_checked_gc_without_host_dependencies() {
    for input in [
        "walz/parser/parser.walz",
        "walz/codegen/compiler.walz",
        "walz/checker/checker.walz",
        "walz/checker/stack.walz",
        "walz/frontend/abi.walz",
    ] {
        let loaded = walz::source::load(input).unwrap();
        walz::check(loaded.text()).unwrap();
    }
    let loaded = walz::source::load("examples/selfhost/portable.walz").unwrap();
    let compiled = walz::compile(loaded.text()).unwrap();
    assert!(!compiled.analysis.opaque);
    assert!(compiled.ir.is_some());
    assert!(!compiled.wat.contains("(import "));
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&compiled.wasm)
        .unwrap();
    let exports = wasmparser::Parser::new(0)
        .parse_all(&compiled.wasm)
        .filter_map(|payload| match payload.unwrap() {
            wasmparser::Payload::ExportSection(exports) => Some(exports),
            _ => None,
        })
        .flatten()
        .map(|export| export.unwrap().name)
        .collect::<Vec<_>>();
    assert_eq!(
        exports,
        [
            "compile",
            "check",
            "compile-project",
            "check-project",
            "imports"
        ],
        "test helpers are not part of the compiler API"
    );
}

#[test]
fn opaque_ir_and_effect_contracts_cannot_be_forged() {
    for (imports, body, expected) in [
        (
            r#"(@use "../walz/checker/checker.walz" (type $module-ir))"#,
            "(func $bad (param $m (ref $module-ir)) (drop (struct.get $module-ir 0 $m)))",
            "opaque",
        ),
        (
            r#"(@use "../walz/parser/parser.walz" (type $syntax))"#,
            "(func $bad (param $m (ref $syntax)) (drop (struct.get $syntax 0 $m)))",
            "opaque",
        ),
        (
            r#"(@use "../walz/frontend/abi.walz" (type $abi-checked))"#,
            "(func $bad (param $c (ref $abi-checked)) (drop (struct.get $abi-checked 0 $c)))",
            "opaque",
        ),
        (
            r#"(@use "../walz/codegen/compiler.walz" (func $compiler.compile))"#,
            "(func $bad (@effect pure) (drop (call $compiler.compile (@bytes.new))))",
            "effect contract",
        ),
        (
            r#"(@use "../walz/core/bytes-builder.walz" (type $bytes-builder))"#,
            "(func $bad (param $m (ref $bytes-builder)) (drop (struct.get $bytes-builder 0 $m)))",
            "opaque",
        ),
        (
            r#"(@use "../walz/checker/ir.walz" (type $instruction-ir))"#,
            "(func $bad (param $n (ref $instruction-ir)) (struct.set $instruction-ir $op $n 0))",
            "immutable",
        ),
        (
            r#"(@use "../walz/checker/ir.walz" (type $gc-type-ir))"#,
            "(func $bad (param $n (ref $gc-type-ir)) (struct.set $gc-type-ir $parent $n 0))",
            "immutable",
        ),
        (
            r#"(@use "../walz/checker/ir.walz" (type $field-ir))"#,
            "(func $bad (param $n (ref $field-ir)) (struct.set $field-ir $mutable $n 1))",
            "immutable",
        ),
    ] {
        let directory = tempfile::tempdir_in(".").unwrap();
        let input = directory.path().join("main.walz");
        std::fs::write(
            &input,
            format!(r#"(module (@use "walz:core" (type $bytes)) {imports} {body})"#),
        )
        .unwrap();
        let loaded = walz::source::load(input).unwrap();
        let error = walz::check(loaded.text()).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn parser_and_codegen_execute_on_wasmtime() {
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .args(["test", "tests/selfhost.test.walz"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("12 passed; 0 failed; 0 errors"));
}
