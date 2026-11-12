//! Small typed rewrites. Full source validation precedes this compiler TCB pass.
use crate::ir::*;

pub(crate) fn simplify(module: &Module) -> Module {
    let kinds = crate::ownership::Kinds::new(module);
    let mut out = module.clone();
    for f in &mut out.functions {
        if let Some(body) = &mut f.body {
            expression(body, module, &kinds);
        }
    }
    out
}

fn expression(e: &mut Expr, module: &Module, kinds: &crate::ownership::Kinds) {
    for child in e.children_mut() {
        expression(child, module, kinds);
    }
    let ExprKind::Match { ty, subject, arms } = &e.kind else {
        return;
    };
    if !matches!(
        module.types[ty.0].kind,
        TypeKind::Value(ValueType::Pair { .. } | ValueType::Result { .. })
    ) || kinds.is_owned(&subject.ty)
    {
        return;
    }
    let ExprKind::VariantNew {
        ty: constructed,
        variant,
        values,
    } = &subject.kind
    else {
        return;
    };
    if constructed != ty {
        return;
    }
    let arm = arms
        .iter()
        .find(|a| a.variant == *variant)
        .expect("checked exhaustive match");
    let mut body = arm.body.clone();
    // Even an unused payload is evaluated once, in original constructor order.
    for (local, value) in arm.bindings.iter().zip(values).rev() {
        body = Expr {
            ty: body.ty.clone(),
            span: e.span,
            kind: ExprKind::Let {
                local: *local,
                value: Box::new(value.clone()),
                body: Box::new(body),
            },
        };
    }
    *e = body;
}
