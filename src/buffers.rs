//! Private packed storage. Mutation is confined to construction; source bytes
//! and text expose no identity, reference casts, or writes.
use crate::ir::*;
mod utf8;

pub(crate) fn storage(ty: &str, value: &str) -> String {
    format!("(struct.get {ty} 0 {value})")
}
pub(crate) fn helpers_with_js_strings(module: &Module, js_strings: bool) -> String {
    let find = |shape| {
        module
            .types
            .iter()
            .position(|t| matches!(&t.kind, TypeKind::Value(v) if *v==shape))
    };
    let (Some(b), Some(t)) = (find(ValueType::Bytes), find(ValueType::Text)) else {
        return String::new();
    };
    let bytes = &module.types[b].name;
    let text = &module.types[t].name;
    let option = find(ValueType::Option(Type::Value(TypeId(t))));
    let mut arrays = std::collections::BTreeMap::new();
    let mut copies = std::collections::BTreeSet::new();
    let (mut validate_whole, mut validate_range, mut decode_range) = (false, false, false);
    for f in &module.functions {
        let mut pending = f.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            if let ExprKind::Buffer { op, args, .. } = &e.kind {
                match *op {
                    BufferOp::FromArray(id)
                        if args.len() == 3
                            || matches!(&module.types[id.0].kind, TypeKind::Array(f) if f.packed.is_some()) =>
                    {
                        let whole = arrays.entry(id).or_insert(false);
                        *whole |= args.len() == 1;
                    }
                    BufferOp::CopyToArray(id) => {
                        copies.insert(id);
                    }
                    BufferOp::IsUtf8 => {
                        validate_whole |= args.len() == 1;
                        validate_range |= args.len() == 3;
                    }
                    BufferOp::Decode => decode_range |= args.len() == 3,
                    _ => {}
                }
            }
            pending.extend(e.children());
        }
    }
    let validator = utf8::helpers(validate_range || decode_range);
    let text_bytes = if js_strings {
        crate::component::js_native::string_helpers(module)
    } else {
        format!(
            r#"      (func $__text_bytes (param $v (ref {text})) (result (ref {bytes}))
        (struct.new {bytes} (struct.get {text} 0 (local.get $v))))"#
        )
    };
    let mut out = format!(
        r#"
      (func $__bytes_slice (param $v (ref {bytes})) (param $start i32) (param $n i32) (result (ref {bytes}))
        (local $a (ref $__byte_storage)) (local $r (ref $__byte_storage))
        (local.set $a (struct.get {bytes} 0 (local.get $v)))
        (if (i32.gt_u (local.get $start) (array.len (local.get $a))) (then (unreachable)))
        (if (i32.gt_u (local.get $n) (i32.sub (array.len (local.get $a)) (local.get $start))) (then (unreachable)))
        (local.set $r (array.new_default $__byte_storage (local.get $n)))
        (array.copy $__byte_storage $__byte_storage
          (local.get $r) (i32.const 0) (local.get $a) (local.get $start) (local.get $n))
        (struct.new {bytes} (local.get $r)))
{text_bytes}
{validator}
    "#
    );
    if validate_whole {
        out.push_str(&format!("(func $__bytes_is_utf8 (param $v (ref {bytes})) (result i32) (call $__utf8 (struct.get {bytes} 0 (local.get $v))))"));
    }
    if validate_range {
        out.push_str(&format!("(func $__bytes_is_utf8_range (param $v (ref {bytes})) (param $start i32) (param $n i32) (result i32) (call $__utf8_range (struct.get {bytes} 0 (local.get $v)) (local.get $start) (local.get $n)))"));
    }
    // Preserve established unpacked conversion output; new bulk/range helpers
    // are emitted only for operations that use them.
    for (id, ty) in module.types.iter().enumerate() {
        if matches!(&ty.kind, TypeKind::Array(field) if field.ty == Type::Num(NumType::I32) && field.packed.is_none())
        {
            let name = &ty.name;
            out.push_str(&format!(
                r#"(func $__bytes_from_array_{id} (param $a (ref {name})) (result (ref {bytes}))
              (local $r (ref $__byte_storage)) (local $i i32) (local $v i32)
              (local.set $r (array.new_default $__byte_storage (array.len (local.get $a))))
              (loop $copy (if (i32.lt_u (local.get $i) (array.len (local.get $a))) (then
                (local.set $v (array.get {name} (local.get $a) (local.get $i)))
                (if (i32.gt_u (local.get $v) (i32.const 255)) (then unreachable))
                (array.set $__byte_storage (local.get $r) (local.get $i) (local.get $v))
                (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
              (struct.new {bytes} (local.get $r)))"#
            ));
        }
    }
    for (id, whole) in arrays {
        let id = id.0;
        let name = &module.types[id].name;
        let TypeKind::Array(field) = &module.types[id].kind else {
            unreachable!()
        };
        let copy = if field.packed == Some(Packed::I8) {
            format!(
                "(array.copy $__byte_storage {name} (local.get $r) (i32.const 0) (local.get $a) (local.get $start) (local.get $n))"
            )
        } else {
            let get = if field.packed.is_some() {
                "array.get_u"
            } else {
                "array.get"
            };
            format!(
                r#"(loop $copy (if (i32.lt_u (local.get $i) (local.get $n)) (then
              (local.set $v ({get} {name} (local.get $a) (i32.add (local.get $start) (local.get $i))))
              (if (i32.gt_u (local.get $v) (i32.const 255)) (then unreachable))
              (array.set $__byte_storage (local.get $r) (local.get $i) (local.get $v))
              (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))"#
            )
        };
        out.push_str(&format!(r#"
          (func $__bytes_from_array_range_{id} (param $a (ref {name})) (param $start i32) (param $n i32) (result (ref {bytes}))
            (local $r (ref $__byte_storage)) (local $i i32) (local $v i32)
            (if (i32.gt_u (local.get $start) (array.len (local.get $a))) (then unreachable))
            (if (i32.gt_u (local.get $n) (i32.sub (array.len (local.get $a)) (local.get $start))) (then unreachable))
            (local.set $r (array.new_default $__byte_storage (local.get $n)))
            {copy}
            (struct.new {bytes} (local.get $r)))"#));
        if whole {
            out.push_str(&format!("(func $__bytes_from_array_{id} (param $a (ref {name})) (result (ref {bytes})) (call $__bytes_from_array_range_{id} (local.get $a) (i32.const 0) (array.len (local.get $a))))"));
        }
    }
    for id in copies {
        let id = id.0;
        let name = &module.types[id].name;
        out.push_str(&format!(r#"(func $__bytes_copy_to_{id} (param $a (ref {name})) (param $at i32) (param $b (ref {bytes})) (param $from i32) (param $n i32)
          (array.copy {name} $__byte_storage (local.get $a) (local.get $at) (struct.get {bytes} 0 (local.get $b)) (local.get $from) (local.get $n)))"#));
    }
    if let Some(option) = option {
        let name = &module.types[option].name;
        if decode_range {
            let value = if js_strings {
                crate::component::js_native::string_from_utf8(module, "(local.get $r)")
            } else {
                format!("(struct.new {text} (local.get $r))")
            };
            out.push_str(&format!(r#"(func $__text_decode_range (param $v (ref {bytes})) (param $start i32) (param $n i32) (result (ref {name}))
              (local $a (ref $__byte_storage)) (local $r (ref $__byte_storage))
              (local.set $a (struct.get {bytes} 0 (local.get $v)))
              (if (result (ref {name})) (call $__utf8_range (local.get $a) (local.get $start) (local.get $n))
                (then
                  (local.set $r (array.new_default $__byte_storage (local.get $n)))
                  (array.copy $__byte_storage $__byte_storage (local.get $r) (i32.const 0) (local.get $a) (local.get $start) (local.get $n))
                  (struct.new $__variant_{option}_1 (i32.const 1) {value}))
                (else (struct.new $__variant_{option}_0 (i32.const 0)))))"#));
        }
        if js_strings {
            out.push_str(&crate::component::js_native::string_decode(module, option));
        } else {
            out.push_str(&format!(r#"(func $__text_decode (param $v (ref {bytes})) (result (ref {name}))
          (if (result (ref {name})) (call $__utf8 (struct.get {bytes} 0 (local.get $v)))
            (then (struct.new $__variant_{option}_1 (i32.const 1) (struct.new {text} (struct.get {bytes} 0 (local.get $v)))))
            (else (struct.new $__variant_{option}_0 (i32.const 0)))))"#));
        }
    }
    out
}
