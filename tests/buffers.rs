#[test]
fn immutable_bytes_and_utf8_text_have_checked_types_and_portable_abi() {
    let loaded = walz::source::load("tests/component-fixtures/buffers.walz").unwrap();
    let source = loaded.text();
    let compiled = walz::compile(source).unwrap();
    assert!(compiled.wat.contains("(array (mut i8))"));
    let wit = walz::wit::emit(compiled.ir.as_ref().unwrap()).unwrap();
    assert!(wit.contains("list<u8>") && wit.contains("string"), "{wit}");
    let component = walz::component::compile(source).unwrap();
    assert!(component.wat.contains("(list u8)"));
}

#[test]
fn byte_slices_lower_to_a_bulk_gc_copy() {
    let loaded = walz::source::load("tests/component-fixtures/buffers.walz").unwrap();
    let source = loaded.text();
    let compiled = walz::compile(source).unwrap();
    // The packed byte storage must stay on the GC heap. A slice should use
    // Wasm's bulk copy rather than a per-byte loop or linear-memory scratch.
    assert!(
        compiled
            .wat
            .contains("array.copy $__byte_storage $__byte_storage")
    );
    assert!(!compiled.wat.contains("(memory "));
}

#[test]
fn buffers_cannot_be_mutated_or_observed_as_references() {
    for body in [
        "(ref.eq (local.get $b) (local.get $b))",
        "(array.len (local.get $b))",
        "(@bytes.get (@string.lit \"a\") (i32.const 0))",
        "(@string.bytes (local.get $b))",
    ] {
        let source = format!("(module (func $f (param $b @bytes) (result i32) {body}))");
        assert!(walz::check(&source).is_err(), "{body}");
    }
}

#[test]
fn buffer_operations_accept_abrupt_operands_without_compiler_panics() {
    for (result, body) in [
        ("i32", "(@bytes.len (unreachable))"),
        ("i32", "(@string.byte-len (unreachable))"),
        ("i32", "(@bytes.get (unreachable) (i32.const 0))"),
        (
            "@bytes",
            "(@bytes.slice (unreachable) (i32.const 0) (i32.const 0))",
        ),
        ("@bytes", "(@string.bytes (unreachable))"),
        ("(@option @string)", "(@string.from-bytes (unreachable))"),
    ] {
        let source = format!("(module (func $f (result {result}) {body}))");
        walz::compile(&source).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
}
