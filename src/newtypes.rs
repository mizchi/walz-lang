//! Representation erasure runs only after nominal type and ownership checking.
use crate::ir::*;
use std::collections::BTreeMap;

fn types(module: &mut Module, f: &mut impl FnMut(&mut Type)) {
    fn value(v: &mut ValueType, f: &mut impl FnMut(&mut Type)) {
        match v {
            ValueType::List(t)
            | ValueType::Option(t)
            | ValueType::Own(t)
            | ValueType::Future(t)
            | ValueType::Stream(t)
            | ValueType::Task(t) => f(t),
            ValueType::Pair { first, second } => {
                f(first);
                f(second);
            }
            ValueType::Result { ok, err } => {
                f(ok);
                f(err);
            }
            _ => {}
        }
    }
    fn expression(e: &mut Expr, f: &mut impl FnMut(&mut Type)) {
        f(&mut e.ty);
        for c in e.children_mut() {
            expression(c, f);
        }
    }
    for t in &mut module.types {
        match &mut t.kind {
            TypeKind::Struct(fields)
            | TypeKind::Closure {
                captures: fields, ..
            } => {
                for field in fields {
                    f(&mut field.ty)
                }
            }
            TypeKind::Array(field) => f(&mut field.ty),
            TypeKind::Variant(variants) => {
                for v in variants {
                    for t in &mut v.payloads {
                        f(t)
                    }
                }
            }
            TypeKind::Value(v) => value(v, f),
            _ => {}
        }
    }
    for fun in &mut module.functions {
        f(&mut fun.result);
        for l in &mut fun.locals {
            f(&mut l.ty);
        }
        if let Some(body) = &mut fun.body {
            expression(body, f);
        }
    }
    for tag in &mut module.tags {
        for t in &mut tag.payloads {
            f(t);
        }
    }
}

pub(crate) fn lower(module: &Module) -> Module {
    if !module
        .types
        .iter()
        .any(|t| matches!(t.kind, TypeKind::Newtype(_)))
    {
        return module.clone();
    }
    let mut out = module.clone();
    types(&mut out, &mut |t| {
        if let Type::Newtype(id) = t {
            if module.types[id.0].abi.is_some() {
                return;
            }
            let TypeKind::Newtype(n) = module.types[id.0].kind else {
                unreachable!()
            };
            *t = Type::Num(n);
        }
    });
    fn erase(e: &mut Expr) {
        for c in e.children_mut() {
            erase(c);
        }
        if let ExprKind::Newtype { value, .. } = &mut e.kind {
            let mut inner = *std::mem::replace(
                value,
                Box::new(Expr {
                    ty: Type::Unit,
                    span: e.span,
                    kind: ExprKind::Nop,
                }),
            );
            inner.span = e.span;
            *e = inner;
        }
    }
    for f in &mut out.functions {
        if let Some(b) = &mut f.body {
            erase(b)
        }
    }
    // Erasure can make option<Id> and option<i32> share a representation. Keep
    // one canonical value type, including generated delivery helper shapes.
    loop {
        let mut seen = BTreeMap::new();
        let mut ids = (0..out.types.len()).map(TypeId).collect::<Vec<_>>();
        for (i, t) in out.types.iter().enumerate() {
            if let TypeKind::Value(v) = &t.kind {
                ids[i] = *seen.entry(v.clone()).or_insert(TypeId(i));
            }
        }
        if ids.iter().enumerate().all(|(i, id)| i == id.0) {
            break;
        }
        // Iterate until all nested value references use canonical IDs.
        let mut changed = false;
        types(&mut out, &mut |t| {
            if let Type::Value(id) = t
                && ids[id.0] != *id
            {
                *id = ids[id.0];
                changed = true;
            }
        });
        fn remap(e: &mut Expr, ids: &[TypeId]) {
            match &mut e.kind {
                ExprKind::VariantNew { ty, .. } | ExprKind::Match { ty, .. } => *ty = ids[ty.0],
                ExprKind::List { list, .. } => *list = ids[list.0],
                ExprKind::Buffer { buffer, .. } => *buffer = ids[buffer.0],
                ExprKind::Delivery { endpoint, .. } => *endpoint = ids[endpoint.0],
                _ => {}
            }
            for c in e.children_mut() {
                remap(c, ids)
            }
        }
        for f in &mut out.functions {
            if let Some(b) = &mut f.body {
                remap(b, &ids)
            }
        }
        if !changed {
            break;
        }
    }
    crate::link::coalesce_values(&out)
}
