//! Resource authority and nonescaping borrows; no general lifetime inference.
use crate::{
    Error,
    bindings::{self, ResourceOperation as Op},
    ir::*,
};

pub(crate) fn validate(source: &str, module: &Module) -> Result<(), Error> {
    let fail = |span, message| Error::at(source, span, message);
    let mut closers = std::collections::BTreeMap::new();
    for f in &module.functions {
        if let Some(r) = bindings::lookup(f).and_then(|b| b.resource)
            && r.operation == Op::Close
            && closers.insert(r.kind, &f.kind).is_some()
        {
            return Err(fail(
                Span::default(),
                "resource kind requires one destructor binding",
            ));
        }
    }
    for f in &module.functions {
        if let Some(r) = bindings::lookup(f).and_then(|b| b.resource)
            && r.operation == Op::New
            && !closers.contains_key(&r.kind)
        {
            return Err(fail(
                Span::default(),
                "resource factory requires a destructor binding",
            ));
        }
    }
    for declaration in &module.types {
        let mut fields = vec![];
        match &declaration.kind {
            TypeKind::Resource { kind, close } => {
                let f = &module.functions[close.0];
                let b = bindings::lookup(f).ok_or_else(|| {
                    fail(
                        Span::default(),
                        "resource destructor requires a trusted binding",
                    )
                })?;
                if b.suspends
                    || b.params != ["i32"]
                    || b.result != "unit"
                    || !b
                        .resource
                        .as_ref()
                        .is_some_and(|r| r.kind == *kind && r.operation == Op::Close)
                {
                    return Err(fail(
                        Span::default(),
                        "resource destructor requires synchronous close(kind, i32) -> unit",
                    ));
                }
            }
            TypeKind::Struct(f) | TypeKind::Closure { captures: f, .. } => {
                fields.extend(f.iter().map(|f| &f.ty))
            }
            TypeKind::Array(f) => fields.push(&f.ty),
            TypeKind::Variant(v) => fields.extend(v.iter().flat_map(|v| &v.payloads)),
            TypeKind::Value(v) => match v {
                ValueType::List(t)
                | ValueType::Option(t)
                | ValueType::Own(t)
                | ValueType::Future(t)
                | ValueType::Stream(t) => fields.push(t),
                ValueType::Result { ok, err } => fields.extend([ok, err]),
                ValueType::Pair { first, second } => fields.extend([first, second]),
                _ => {}
            },
        }
        if fields.iter().any(|t| matches!(t, Type::Borrow(_))) {
            return Err(fail(Span::default(), "borrow cannot be stored or captured"));
        }
    }
    let kinds = crate::ownership::Kinds::new(module);
    for f in &module.functions {
        if matches!(f.result, Type::Borrow(_))
            || module
                .tags
                .iter()
                .any(|t| t.payloads.iter().any(|t| matches!(t, Type::Borrow(_))))
        {
            return Err(fail(Span::default(), "borrow cannot be returned or thrown"));
        }
        for ty in f.locals.iter().map(|l| &l.ty).chain([&f.result]) {
            match ty {
                Type::Borrow(id) | Type::Resource(id)
                    if !matches!(module.types[id.0].kind, TypeKind::Resource { .. }) =>
                {
                    return Err(fail(Span::default(), "expected resource type"));
                }
                Type::Ref { ty, .. }
                    if matches!(module.types[ty.0].kind, TypeKind::Resource { .. }) =>
                {
                    return Err(fail(
                        Span::default(),
                        "resource representation is private; use resource or borrow",
                    ));
                }
                _ => {}
            }
        }
        let mut pending = f.body.iter().map(|e| (e, None::<&str>)).collect::<Vec<_>>();
        while let Some((e, handle_kind)) = pending.pop() {
            match &e.kind {
                ExprKind::ResourceNew { ty, handle } => {
                    let TypeKind::Resource { kind, .. } = &module.types[ty.0].kind else {
                        unreachable!()
                    };
                    if !matches!(&handle.kind,ExprKind::Call{function,..} if bindings::lookup(&module.functions[function.0]).and_then(|b|b.resource).is_some_and(|r|r.kind==*kind && r.operation==Op::New))
                    {
                        return Err(fail(
                            e.span,
                            "resource.new requires a matching resource factory call",
                        ));
                    }
                }
                ExprKind::ResourceHandle(value) => {
                    let Type::Borrow(id) = value.ty else {
                        unreachable!()
                    };
                    let TypeKind::Resource { kind, .. } = &module.types[id.0].kind else {
                        return Err(fail(e.span, "expected resource type"));
                    };
                    if handle_kind != Some(kind) {
                        return Err(fail(
                            e.span,
                            "resource.handle is only allowed as the first argument of a matching host operation",
                        ));
                    }
                }
                ExprKind::Using { body, .. } => {
                    if kinds.is_owned(&body.ty) || matches!(body.ty, Type::Borrow(_)) {
                        return Err(fail(
                            e.span,
                            "using cannot return a borrow or an owned value",
                        ));
                    }
                    let mut nested = vec![body.as_ref()];
                    let mut labels = std::collections::BTreeSet::new();
                    let mut branches = vec![];
                    while let Some(e) = nested.pop() {
                        if let ExprKind::Loop { label, .. } = e.kind {
                            labels.insert(label);
                        }
                        if let ExprKind::Recur { label, .. } = e.kind {
                            branches.push(label);
                        }
                        nested.extend(e.children());
                    }
                    if branches.iter().any(|label| !labels.contains(label)) {
                        return Err(fail(
                            e.span,
                            "recur cannot leave a using scope; close before recurring",
                        ));
                    }
                }
                ExprKind::Call { function, args } => {
                    let b = bindings::lookup(&module.functions[function.0]);
                    for (i, arg) in args.iter().enumerate() {
                        let kind = if i == 0 {
                            b.as_ref()
                                .and_then(|b| b.resource.as_ref())
                                .filter(|r| r.operation == Op::Use)
                                .map(|r| r.kind.clone())
                        } else {
                            None
                        };
                        // Process this special edge immediately: policy strings live outside the traversal.
                        if let ExprKind::ResourceHandle(value) = &arg.kind {
                            let Type::Borrow(id) = value.ty else {
                                unreachable!()
                            };
                            let TypeKind::Resource { kind: expected, .. } =
                                &module.types[id.0].kind
                            else {
                                return Err(fail(arg.span, "expected resource type"));
                            };
                            if kind.as_ref() != Some(expected) {
                                return Err(fail(
                                    arg.span,
                                    "resource.handle requires a matching host resource operation",
                                ));
                            }
                            pending.push((value, None));
                        } else {
                            pending.push((arg, None));
                        }
                    }
                    continue;
                }
                _ => {}
            }
            pending.extend(e.children().into_iter().map(|e| (e, None)));
        }
    }
    Ok(())
}
