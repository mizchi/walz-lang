//! Private GC list storage; source operations never expose a mutable array.
use crate::ir::*;
use std::collections::BTreeSet;

pub(crate) fn expression(module: &Module, op: ListOp, id: TypeId, args: &[String]) -> String {
    let n = id.0;
    let name = &module.types[n].name;
    let TypeKind::Value(ValueType::List(payload)) = &module.types[n].kind else {
        unreachable!()
    };
    match op {
        ListOp::New => {
            let values = args
                .iter()
                .map(|v| {
                    if *payload == Type::Unit {
                        format!(" (block (result i32) {v} (i32.const 0))")
                    } else {
                        format!(" {v}")
                    }
                })
                .collect::<String>();
            format!(
                "(struct.new {name} (array.new_fixed $__list_storage_{n} {}{values}))",
                args.len()
            )
        }
        ListOp::Len => format!("(array.len (struct.get {name} 0 {}))", args[0]),
        ListOp::Get => format!("(call $__list_get_{n} {} {})", args[0], args[1]),
        ListOp::Slice => format!(
            "(call $__list_slice_{n} {} {} {})",
            args[0], args[1], args[2]
        ),
        ListOp::FromArray(array) => format!("(call $__list_from_{n}_{} {})", array.0, args[0]),
    }
}

pub(crate) fn helpers(module: &Module) -> String {
    let mut out = String::new();
    let mut arrays = BTreeSet::new();
    for f in &module.functions {
        let mut pending = f.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            if let ExprKind::List {
                op: ListOp::FromArray(array),
                list,
                ..
            } = e.kind
            {
                arrays.insert((list, array));
            }
            pending.extend(e.children());
        }
    }
    for (n, t) in module.types.iter().enumerate() {
        let TypeKind::Value(ValueType::List(payload)) = &t.kind else {
            continue;
        };
        let name = &t.name;
        let storage = format!("$__list_storage_{n}");
        let (result, get) = if *payload == Type::Unit {
            (
                String::new(),
                format!(
                    "(drop (array.get_u {storage} (struct.get {name} 0 (local.get $xs)) (local.get $i)))"
                ),
            )
        } else {
            (
                format!(" (result {})", crate::emit::ty(module, payload)),
                format!(
                    "(array.get {storage} (struct.get {name} 0 (local.get $xs)) (local.get $i))"
                ),
            )
        };
        out.push_str(&format!(
            "(func $__list_get_{n} (param $xs (ref {name})) (param $i i32){result} {get})\n"
        ));
        let initial = if *payload == Type::Unit {
            "(i32.const 0)".into()
        } else {
            format!("(array.get {storage} (local.get $source) (local.get $start))")
        };
        out.push_str(&format!("(func $__list_slice_{n} (param $xs (ref {name})) (param $start i32) (param $len i32) (result (ref {name}))
          (local $source (ref {storage})) (local $copy (ref {storage}))
          (local.set $source (struct.get {name} 0 (local.get $xs)))
          (if (i32.or (i32.gt_u (local.get $start) (array.len (local.get $source))) (i32.gt_u (local.get $len) (i32.sub (array.len (local.get $source)) (local.get $start)))) (then (unreachable)))
          (local.set $copy (if (result (ref {storage})) (local.get $len)
            (then (array.new {storage} {initial} (local.get $len))) (else (array.new_fixed {storage} 0))))
          (array.copy {storage} {storage} (local.get $copy) (i32.const 0) (local.get $source) (local.get $start) (local.get $len))
          (struct.new {name} (local.get $copy)))\n"));
    }
    for (list, array) in arrays {
        let n = list.0;
        let name = &module.types[n].name;
        let source = &module.types[array.0].name;
        let storage = format!("$__list_storage_{n}");
        out.push_str(&format!("(func $__list_from_{n}_{} (param $source (ref {source})) (result (ref {name}))
          (local $len i32) (local $copy (ref {storage}))
          (local.set $len (array.len (local.get $source)))
          (local.set $copy (if (result (ref {storage})) (local.get $len)
            (then (array.new {storage} (array.get {source} (local.get $source) (i32.const 0)) (local.get $len))) (else (array.new_fixed {storage} 0))))
          (array.copy {storage} {source} (local.get $copy) (i32.const 0) (local.get $source) (i32.const 0) (local.get $len))
          (struct.new {name} (local.get $copy)))\n",array.0));
    }
    out
}
