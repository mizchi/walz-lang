//! JS strings retain a lazy immutable UTF-8 cache. Byte offsets, BOM and invalid
//! UTF-8 behavior stay identical to the portable backend.
use crate::ir::*;
use std::collections::BTreeMap;

pub(crate) fn literals(module: &Module) -> BTreeMap<Vec<u8>, usize> {
    let mut values = BTreeMap::new();
    for f in &module.functions {
        let mut stack = f.body.iter().collect::<Vec<_>>();
        while let Some(e) = stack.pop() {
            if let ExprKind::Data(bytes) = &e.kind
                && matches!(e.ty,Type::Value(id) if matches!(module.types[id.0].kind,TypeKind::Value(ValueType::Text)))
            {
                let n = values.len();
                values.entry(bytes.clone()).or_insert(n);
            }
            stack.extend(e.children());
        }
    }
    values
}
pub(crate) fn imports(values: &BTreeMap<Vec<u8>, usize>) -> String {
    let mut out="(import \"wasm:js-string\" \"cast\" (func $__js_string_cast (param externref) (result (ref extern))))\n(import \"__native\" \"encode\" (func $__native_host_encode (param externref) (result i32)))\n(import \"__native\" \"decode\" (func $__native_host_decode (param i32) (result externref)))\n".to_owned();
    for (value, i) in values {
        out += &format!(
            "(import \"walz:strings\" {} (global $__js_literal_{i} externref))\n",
            crate::emit::quoted(std::str::from_utf8(value).expect("checked UTF-8 literal"))
        );
    }
    out
}
pub(crate) fn helpers(module: &Module) -> String {
    let Some(bytes) = module
        .types
        .iter()
        .find(|t| matches!(t.kind, TypeKind::Value(ValueType::Bytes)))
        .map(|t| t.name.as_str())
    else {
        return String::new();
    };
    let Some(text) = module
        .types
        .iter()
        .find(|t| matches!(t.kind, TypeKind::Value(ValueType::Text)))
        .map(|t| t.name.as_str())
    else {
        return String::new();
    };
    format!(
        r#"
      (func $__text_bytes (param $v (ref {text})) (result (ref {bytes}))
        (local $a (ref null $__byte_storage))
        (local.set $a (struct.get {text} 1 (local.get $v)))
        (if (ref.is_null (local.get $a)) (then
          (local.set $a (call $__native_from_memory (call $__native_host_encode (struct.get {text} 0 (local.get $v)))))
          (struct.set {text} 1 (local.get $v) (local.get $a))))
        (struct.new {bytes} (ref.as_non_null (local.get $a))))
    "#
    )
}
pub(crate) fn decode(module: &Module, option: usize) -> String {
    let bytes = module
        .types
        .iter()
        .find(|t| matches!(t.kind, TypeKind::Value(ValueType::Bytes)))
        .unwrap()
        .name
        .as_str();
    let text = module
        .types
        .iter()
        .find(|t| matches!(t.kind, TypeKind::Value(ValueType::Text)))
        .unwrap()
        .name
        .as_str();
    let name = &module.types[option].name;
    format!(
        r#"(func $__text_decode (param $v (ref {bytes})) (result (ref {name}))
      (if (result (ref {name})) (call $__utf8 (struct.get {bytes} 0 (local.get $v)))
        (then (struct.new $__variant_{option}_1 (i32.const 1)
          (struct.new {text}
            (call $__js_string_cast (call $__native_host_decode (call $__native_to_memory (struct.get {bytes} 0 (local.get $v)))))
            (struct.get {bytes} 0 (local.get $v)))))
        (else (struct.new $__variant_{option}_0 (i32.const 0)))))"#
    )
}

/// Internal construction only after strict guest validation. The storage is an
/// independent immutable UTF-8 snapshot, including any leading BOM.
pub(crate) fn from_utf8(module: &Module, storage: &str) -> String {
    let text = &module.types[module.value_id(&ValueType::Text).0].name;
    format!(
        r#"(struct.new {text}
      (call $__js_string_cast (call $__native_host_decode (call $__native_to_memory {storage})))
      {storage})"#
    )
}

pub(super) fn transfer_helpers() -> &'static str {
    r#"
      (memory $__native_memory (export "__transfer") 1 256)
      (func $__native_reserve (param $n i32) (local $pages i32)
        (if (i32.gt_u (local.get $n) (i32.const 16777216)) (then unreachable))
        (local.set $pages (i32.div_u (i32.add (local.get $n) (i32.const 65535)) (i32.const 65536)))
        (if (i32.gt_u (local.get $pages) (memory.size)) (then
          (if (i32.eq (memory.grow (i32.sub (local.get $pages) (memory.size))) (i32.const -1)) (then unreachable)))))
      (func $__native_from_memory (param $n i32) (result (ref $__byte_storage))
        (local $a (ref $__byte_storage)) (local $i i32)
        (if (i32.gt_u (local.get $n) (i32.const 16777216)) (then unreachable))
        (if (i32.gt_u (local.get $n) (i32.mul (memory.size) (i32.const 65536))) (then unreachable))
        (local.set $a (array.new_default $__byte_storage (local.get $n)))
        (loop $copy (if (i32.lt_u (local.get $i) (local.get $n)) (then
          (array.set $__byte_storage (local.get $a) (local.get $i) (i32.load8_u (local.get $i)))
          (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
        (local.get $a))
      (func $__native_to_memory (param $a (ref $__byte_storage)) (result i32)
        (local $i i32) (local $n i32)
        (local.set $n (array.len (local.get $a))) (call $__native_reserve (local.get $n))
        (loop $copy (if (i32.lt_u (local.get $i) (local.get $n)) (then
          (i32.store8 (local.get $i) (array.get_u $__byte_storage (local.get $a) (local.get $i)))
          (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
        (local.get $n))
    "#
}
