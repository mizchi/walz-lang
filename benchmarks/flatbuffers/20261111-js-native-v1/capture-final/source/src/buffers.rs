//! Private packed storage. Mutation is confined to construction; source bytes
//! and text expose no identity, reference casts, or writes.
use crate::ir::*;

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
    // Entered only at a character boundary with an ASCII first byte. Stop
    // before the first non-ASCII byte so mixed text never causes overlapping
    // lookahead scans. The caller proves that all eight indices are in bounds.
    let ascii = (1..8).map(|offset| format!(
        "(if (i32.ge_u (array.get_u $__byte_storage (local.get $a) (i32.add (local.get $i) (i32.const {offset}))) (i32.const 128)) (then (local.set $i (i32.add (local.get $i) (i32.const {offset}))) (br $next)))"
    )).collect::<String>();
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
      (func $__utf8 (param $a (ref $__byte_storage)) (result i32)
        (local $i i32) (local $b i32) (local $remaining i32) (local $lo i32) (local $hi i32)
        (local.set $lo (i32.const 128)) (local.set $hi (i32.const 191))
        (loop $next
          (if (i32.lt_u (local.get $i) (array.len (local.get $a))) (then
            (local.set $b (array.get_u $__byte_storage (local.get $a) (local.get $i)))
            (if (local.get $remaining) (then
              (if (i32.or (i32.lt_u (local.get $b) (local.get $lo)) (i32.gt_u (local.get $b) (local.get $hi))) (then (return (i32.const 0))))
              (local.set $remaining (i32.sub (local.get $remaining) (i32.const 1)))
              (local.set $lo (i32.const 128)) (local.set $hi (i32.const 191)))
            (else
              (if (i32.ge_u (local.get $b) (i32.const 128)) (then
                (if (i32.or (i32.lt_u (local.get $b) (i32.const 194)) (i32.gt_u (local.get $b) (i32.const 244))) (then (return (i32.const 0))))
                (local.set $remaining (i32.const 1))
                (if (i32.ge_u (local.get $b) (i32.const 224)) (then (local.set $remaining (i32.const 2))))
                (if (i32.ge_u (local.get $b) (i32.const 240)) (then (local.set $remaining (i32.const 3))))
                (if (i32.eq (local.get $b) (i32.const 224)) (then (local.set $lo (i32.const 160))))
                (if (i32.eq (local.get $b) (i32.const 237)) (then (local.set $hi (i32.const 159))))
                (if (i32.eq (local.get $b) (i32.const 240)) (then (local.set $lo (i32.const 144))))
                (if (i32.eq (local.get $b) (i32.const 244)) (then (local.set $hi (i32.const 143)))))
              (else
                (if (i32.ge_u (i32.sub (array.len (local.get $a)) (local.get $i)) (i32.const 8)) (then
                  {ascii}
                  (local.set $i (i32.add (local.get $i) (i32.const 8))) (br $next)))))))
            (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $next))))
        (i32.eqz (local.get $remaining)))
    "#
    );
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
    let mut arrays = std::collections::BTreeMap::new();
    let mut copies = std::collections::BTreeSet::new();
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
                    _ => {}
                }
            }
            pending.extend(e.children());
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
