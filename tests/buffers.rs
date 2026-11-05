#[test]
fn immutable_bytes_and_utf8_text_have_checked_types_and_portable_abi() {
    let source = include_str!("component-fixtures/buffers.wlt");
    let compiled = wlt::compile(source).unwrap();
    assert!(compiled.wat.contains("(array (mut i8))"));
    let wit = wlt::wit::emit(compiled.ir.as_ref().unwrap()).unwrap();
    assert!(wit.contains("list<u8>") && wit.contains("string"), "{wit}");
    let component = wlt::component::compile(source).unwrap();
    assert!(component.wat.contains("(list u8)"));
}

#[test]
fn buffers_cannot_be_mutated_or_observed_as_references() {
    for body in [
        "(ref.eq (local.get $b) (local.get $b))",
        "(array.len (local.get $b))",
        "(%bytes.get (%text.lit \"a\") (i32.const 0))",
        "(%text.bytes (local.get $b))",
    ] {
        let source = format!("(module (func $f (param $b %bytes) (result i32) {body}))");
        assert!(wlt::check(&source).is_err(), "{body}");
    }
}

#[test]
fn buffer_operations_accept_abrupt_operands_without_compiler_panics() {
    for (result, body) in [
        ("i32", "(%bytes.len (unreachable))"),
        ("i32", "(%text.byte-len (unreachable))"),
        ("i32", "(%bytes.get (unreachable) (i32.const 0))"),
        (
            "%bytes",
            "(%bytes.slice (unreachable) (i32.const 0) (i32.const 0))",
        ),
        ("%bytes", "(%text.bytes (unreachable))"),
        ("(%option %text)", "(%text.from-bytes (unreachable))"),
    ] {
        let source = format!("(module (func $f (result {result}) {body}))");
        wlt::compile(&source).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
}
