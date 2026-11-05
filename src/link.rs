//! Declaration reachability after whole-source validation. No expression is
//! rewritten or reordered. Native WAT does not pass through this projection.
use crate::ir::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Item {
    Function(usize),
    Type(usize),
    Tag(usize),
}

fn add_type(ty: &Type, pending: &mut Vec<Item>) {
    if let Type::Ref { ty, .. } | Type::Value(ty) | Type::Resource(ty) | Type::Borrow(ty) = ty {
        pending.push(Item::Type(ty.0));
    }
}

pub(crate) fn project(module: &Module) -> Module {
    let mut pending = module
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.exports.is_empty())
        .map(|(i, _)| Item::Function(i))
        .collect::<Vec<_>>();
    pending.extend(module.laws.iter().map(|law| Item::Function(law.function.0)));
    let mut live = BTreeSet::new();
    while let Some(item) = pending.pop() {
        if !live.insert(item) {
            continue;
        }
        match item {
            Item::Function(i) => {
                let f = &module.functions[i];
                for local in &f.locals {
                    add_type(&local.ty, &mut pending);
                }
                add_type(&f.result, &mut pending);
                if let FunctionKind::Closure { environment } = f.kind {
                    pending.push(Item::Type(environment.0));
                }
                if let Some(contract) = &f.effect_contract {
                    pending.extend(contract.allowed.throws.iter().map(|id| Item::Tag(id.0)));
                }
                let mut expressions = f.body.iter().collect::<Vec<_>>();
                while let Some(e) = expressions.pop() {
                    add_type(&e.ty, &mut pending);
                    use ExprKind::*;
                    match &e.kind {
                        Call { function, .. } => pending.push(Item::Function(function.0)),
                        Buffer { buffer, op, .. } => {
                            pending.push(Item::Type(buffer.0));
                            if let BufferOp::FromArray(id) = op {
                                pending.push(Item::Type(id.0));
                            }
                        }
                        Delivery { endpoint, .. } => pending.push(Item::Type(endpoint.0)),
                        Throw { tag, .. } => pending.push(Item::Tag(tag.0)),
                        Try { handlers, .. } => pending.extend(
                            handlers
                                .iter()
                                .filter_map(|h| h.tag)
                                .map(|id| Item::Tag(id.0)),
                        ),
                        ResourceNew { ty, .. }
                        | VariantNew { ty, .. }
                        | Match { ty, .. }
                        | StructNew { ty, .. }
                        | StructGet { ty, .. }
                        | StructSet { ty, .. }
                        | ArrayNew { ty, .. }
                        | ArrayFixed { ty, .. }
                        | ArrayGet { ty, .. }
                        | ArraySet { ty, .. }
                        | Null(ty) => pending.push(Item::Type(ty.0)),
                        _ => {}
                    }
                    expressions.extend(e.children());
                }
            }
            Item::Type(i) => {
                match &module.types[i].kind {
                    TypeKind::Resource { close, .. } => pending.push(Item::Function(close.0)),
                    TypeKind::Struct(fields) => {
                        for f in fields {
                            add_type(&f.ty, &mut pending);
                        }
                    }
                    TypeKind::Array(field) => add_type(&field.ty, &mut pending),
                    TypeKind::Closure { captures, function } => {
                        pending.push(Item::Function(function.0));
                        for f in captures {
                            add_type(&f.ty, &mut pending);
                        }
                    }
                    TypeKind::Variant(variants) => {
                        for v in variants {
                            for t in &v.payloads {
                                add_type(t, &mut pending);
                            }
                        }
                    }
                    TypeKind::Value(value) => {
                        if let ValueType::Stream(payload) = value {
                            let option = module.value_id(&ValueType::Option(payload.clone()));
                            let pair = module.value_id(&ValueType::Pair {
                                first: Type::Value(TypeId(i)),
                                second: Type::Value(option),
                            });
                            pending.extend([Item::Type(option.0), Item::Type(pair.0)]);
                        }
                        // The shared buffer helpers use both private wrappers.
                        if matches!(value, ValueType::Bytes | ValueType::Text) {
                            pending.extend(
                                module
                                    .types
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, t)| {
                                        matches!(
                                            t.kind,
                                            TypeKind::Value(ValueType::Bytes | ValueType::Text)
                                        )
                                    })
                                    .map(|(i, _)| Item::Type(i)),
                            );
                        }
                        for v in value.variants(TypeId(i)) {
                            for t in &v.payloads {
                                add_type(t, &mut pending);
                            }
                        }
                    }
                }
            }
            Item::Tag(i) => {
                for t in &module.tags[i].payloads {
                    add_type(t, &mut pending);
                }
            }
        }
    }
    let map = |len, item: fn(usize) -> Item| {
        let mut next = 0;
        (0..len)
            .map(|i| {
                if live.contains(&item(i)) {
                    let n = next;
                    next += 1;
                    n
                } else {
                    usize::MAX
                }
            })
            .collect::<Vec<_>>()
    };
    let ids = Ids {
        functions: map(module.functions.len(), Item::Function),
        types: map(module.types.len(), Item::Type),
        tags: map(module.tags.len(), Item::Tag),
    };
    let mut out = module.clone();
    out.functions = out
        .functions
        .into_iter()
        .enumerate()
        .filter(|(i, _)| live.contains(&Item::Function(*i)))
        .map(|(_, mut f)| {
            for local in &mut f.locals {
                ids.ty(&mut local.ty);
            }
            ids.ty(&mut f.result);
            if let FunctionKind::Closure { environment } = &mut f.kind {
                ids.type_id(environment);
            }
            if let Some(contract) = &mut f.effect_contract {
                contract.allowed.throws = contract
                    .allowed
                    .throws
                    .iter()
                    .map(|id| TagId(ids.tags[id.0]))
                    .collect();
            }
            if let Some(body) = &mut f.body {
                ids.expression(body);
            }
            f
        })
        .collect();
    out.types = out
        .types
        .into_iter()
        .enumerate()
        .filter(|(i, _)| live.contains(&Item::Type(*i)))
        .map(|(_, mut t)| {
            match &mut t.kind {
                TypeKind::Struct(fields) => {
                    for f in fields {
                        ids.ty(&mut f.ty);
                    }
                }
                TypeKind::Resource { close, .. } => close.0 = ids.functions[close.0],
                TypeKind::Array(field) => ids.ty(&mut field.ty),
                TypeKind::Closure { captures, function } => {
                    function.0 = ids.functions[function.0];
                    for f in captures {
                        ids.ty(&mut f.ty);
                    }
                }
                TypeKind::Variant(variants) => {
                    for v in variants {
                        for t in &mut v.payloads {
                            ids.ty(t);
                        }
                    }
                }
                TypeKind::Value(value) => match value {
                    ValueType::Bytes | ValueType::Text => {}
                    ValueType::Option(t)
                    | ValueType::Own(t)
                    | ValueType::Future(t)
                    | ValueType::Stream(t) => ids.ty(t),
                    ValueType::Result { ok, err } => {
                        ids.ty(ok);
                        ids.ty(err);
                    }
                    ValueType::Pair { first, second } => {
                        ids.ty(first);
                        ids.ty(second);
                    }
                },
            }
            t
        })
        .collect();
    out.tags = out
        .tags
        .into_iter()
        .enumerate()
        .filter(|(i, _)| live.contains(&Item::Tag(*i)))
        .map(|(_, mut tag)| {
            for t in &mut tag.payloads {
                ids.ty(t);
            }
            tag
        })
        .collect();
    for test in &mut out.tests {
        test.function.0 = ids.functions[test.function.0];
    }
    for law in &mut out.laws {
        law.function.0 = ids.functions[law.function.0];
    }
    out
}

struct Ids {
    functions: Vec<usize>,
    types: Vec<usize>,
    tags: Vec<usize>,
}
impl Ids {
    fn type_id(&self, id: &mut TypeId) {
        id.0 = self.types[id.0];
    }
    fn ty(&self, ty: &mut Type) {
        if let Type::Ref { ty, .. } | Type::Value(ty) | Type::Resource(ty) | Type::Borrow(ty) = ty {
            self.type_id(ty);
        }
    }
    fn expression(&self, e: &mut Expr) {
        self.ty(&mut e.ty);
        use ExprKind::*;
        match &mut e.kind {
            Call { function, .. } => function.0 = self.functions[function.0],
            Buffer { buffer, op, .. } => {
                self.type_id(buffer);
                if let BufferOp::FromArray(id) = op {
                    self.type_id(id);
                }
            }
            Delivery { endpoint, .. } => self.type_id(endpoint),
            Throw { tag, .. } => tag.0 = self.tags[tag.0],
            Try { handlers, .. } => {
                for handler in handlers {
                    if let Some(tag) = &mut handler.tag {
                        tag.0 = self.tags[tag.0];
                    }
                }
            }
            ResourceNew { ty, .. }
            | VariantNew { ty, .. }
            | Match { ty, .. }
            | StructNew { ty, .. }
            | StructGet { ty, .. }
            | StructSet { ty, .. }
            | ArrayNew { ty, .. }
            | ArrayFixed { ty, .. }
            | ArrayGet { ty, .. }
            | ArraySet { ty, .. }
            | Null(ty) => self.type_id(ty),
            _ => {}
        }
        for child in e.children_mut() {
            self.expression(child);
        }
    }
}
