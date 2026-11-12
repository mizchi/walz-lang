//! Consumer fusion for private shared Pair/Option/Result storage. A container
//! must be constructed here and used only by matches/drops, or matched directly.
//! Payloads are evaluated once before the continuation, including defaults.
//! Nothing crosses a try/label/loop producer scope or changes public signatures.
use super::inline;
use crate::ir::*;

enum Consumer {
    Match { arms: Vec<MatchArm>, ty: Type },
    Local { local: LocalId, body: Expr },
    Drop { span: Span },
}
impl Consumer {
    fn ty(&self) -> Type {
        match self {
            Self::Match { ty, .. } => ty.clone(),
            Self::Local { body, .. } => body.ty.clone(),
            Self::Drop { .. } => Type::Unit,
        }
    }
    fn cost(&self) -> usize {
        match self {
            Self::Match { arms, .. } => arms.iter().map(|a| inline::nodes(&a.body).len()).sum(),
            Self::Local { body, .. } => inline::nodes(body).len(),
            Self::Drop { .. } => 1,
        }
    }
}

pub(super) fn simplify(module: &Module) -> Module {
    let kinds = crate::ownership::Kinds::new(module);
    let plan = inline::Plan::values(module, &kinds);
    let mut out = module.clone();
    for (index, function) in out.functions.iter_mut().enumerate() {
        if !inline::shared(function, &kinds) || function.locals.len() > 2048 {
            continue;
        }
        let Some(body) = &function.body else { continue };
        if inline::nodes(body).len() > 4096 {
            continue;
        }
        let original = function.clone();
        *function = plan.expand(module, index);
        let mut visitor = Visitor {
            module,
            kinds: &kinds,
            locals: &mut function.locals,
            budget: 4096,
            removed: 0,
        };
        // A replacement may expose a nested container in one of its payload lets.
        for _ in 0..4 {
            let before = visitor.removed;
            visitor.walk(function.body.as_mut().unwrap(), 0);
            if before == visitor.removed {
                break;
            }
        }
        if visitor.removed == 0 {
            *function = original;
        }
    }
    out
}

struct Visitor<'a> {
    module: &'a Module,
    kinds: &'a crate::ownership::Kinds,
    locals: &'a mut Vec<Binding>,
    budget: usize,
    removed: usize,
}
impl Visitor<'_> {
    fn private(&self, ty: &Type) -> bool {
        matches!(ty, Type::Value(id) if inline::private_value(self.module, *id))
            && !self.kinds.is_owned(ty)
    }
    fn walk(&mut self, e: &mut Expr, depth: usize) {
        if depth >= 128 || self.budget == 0 {
            return;
        }
        for child in e.children_mut() {
            self.walk(child, depth + 1);
        }
        let attempt = match &e.kind {
            ExprKind::Match { subject, arms, .. } if self.private(&subject.ty) => Some((
                subject.as_ref(),
                Consumer::Match {
                    arms: arms.clone(),
                    ty: e.ty.clone(),
                },
            )),
            ExprKind::Let { local, value, body }
                if self.private(&value.ty) && consumed_only(body, *local) =>
            {
                Some((
                    value.as_ref(),
                    Consumer::Local {
                        local: *local,
                        body: body.as_ref().clone(),
                    },
                ))
            }
            ExprKind::Drop(value) if self.private(&value.ty) => {
                Some((value.as_ref(), Consumer::Drop { span: e.span }))
            }
            _ => None,
        };
        let Some((producer, consumer)) = attempt else {
            return;
        };
        let locals = self.locals.len();
        let removed = self.removed;
        if let Some(replacement) = self.fuse(producer, &consumer, 0)
            && self.removed > removed
        {
            *e = replacement;
            return;
        }
        self.locals.truncate(locals);
        self.removed = removed;
    }
    fn fuse(&mut self, producer: &Expr, consumer: &Consumer, depth: usize) -> Option<Expr> {
        if depth >= 32 || self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        let kind = match &producer.kind {
            ExprKind::VariantNew {
                ty,
                variant,
                values,
            } if inline::private_value(self.module, *ty) && self.private(&producer.ty) => {
                let cost = consumer.cost()
                    + values
                        .iter()
                        .map(|v| inline::nodes(v).len() + 2)
                        .sum::<usize>();
                self.budget = self.budget.checked_sub(cost)?;
                let mut bindings = Vec::new();
                let mut payloads = Vec::new();
                for value in values {
                    let id = LocalId(self.locals.len());
                    self.locals.push(Binding {
                        name: format!("$__scalar_{}", id.0),
                        span: value.span,
                        ty: value.ty.clone(),
                        kind: BindingKind::Let,
                    });
                    bindings.push((id, value.clone()));
                    payloads.push(Expr {
                        ty: value.ty.clone(),
                        span: value.span,
                        kind: ExprKind::Local(id),
                    });
                }
                let mut body = match consumer {
                    Consumer::Match { arms, .. } => select(arms, *variant, &payloads),
                    Consumer::Local { local, body } => {
                        let mut body = body.clone();
                        replace_consumers(&mut body, *local, *variant, &payloads);
                        body
                    }
                    Consumer::Drop { span } => Expr {
                        ty: Type::Unit,
                        span: *span,
                        kind: ExprKind::Nop,
                    },
                };
                for (local, value) in bindings.into_iter().rev() {
                    body = Expr {
                        ty: body.ty.clone(),
                        span: producer.span,
                        kind: ExprKind::Let {
                            local,
                            value: Box::new(value),
                            body: Box::new(body),
                        },
                    };
                }
                self.removed += 1;
                return Some(body);
            }
            ExprKind::Let { local, value, body } => ExprKind::Let {
                local: *local,
                value: value.clone(),
                body: Box::new(self.fuse(body, consumer, depth + 1)?),
            },
            ExprKind::Block(items) if !items.is_empty() => {
                let mut out = items[..items.len() - 1].to_vec();
                out.push(self.fuse(items.last().unwrap(), consumer, depth + 1)?);
                ExprKind::Block(out)
            }
            ExprKind::If {
                condition,
                then_body,
                else_body,
            } => ExprKind::If {
                condition: condition.clone(),
                then_body: Box::new(self.fuse(then_body, consumer, depth + 1)?),
                else_body: Box::new(self.fuse(else_body, consumer, depth + 1)?),
            },
            ExprKind::Match { ty, subject, arms } if !self.kinds.is_owned(&subject.ty) => {
                let mut out = Vec::new();
                for arm in arms {
                    out.push(MatchArm {
                        variant: arm.variant,
                        bindings: arm.bindings.clone(),
                        body: self.fuse(&arm.body, consumer, depth + 1)?,
                    });
                }
                ExprKind::Match {
                    ty: *ty,
                    subject: subject.clone(),
                    arms: out,
                }
            }
            // These paths never reach the consumer. Preserve bottom typing.
            ExprKind::Unreachable | ExprKind::Throw { .. } | ExprKind::Return(_) => {
                return Some(producer.clone());
            }
            _ => return None,
        };
        Some(Expr {
            ty: consumer.ty(),
            span: producer.span,
            kind,
        })
    }
}

/// Reject escapes through calls, returns, stores, aliases and closure captures.
/// Multiple matches/drops of the same snapshot are allowed, including in loops.
fn consumed_only(body: &Expr, local: LocalId) -> bool {
    let mut pending = vec![body];
    while let Some(e) = pending.pop() {
        match &e.kind {
            ExprKind::Local(id) if *id == local => return false,
            ExprKind::Match { subject, arms, .. } if matches!(subject.kind, ExprKind::Local(id) if id == local) =>
            {
                pending.extend(arms.iter().map(|a| &a.body));
            }
            ExprKind::Drop(value) if matches!(value.kind, ExprKind::Local(id) if id == local) => {
                // The container itself is discarded; its payloads still run.
            }
            _ => pending.extend(e.children()),
        }
    }
    true
}

fn select(arms: &[MatchArm], variant: usize, values: &[Expr]) -> Expr {
    let arm = arms
        .iter()
        .find(|a| a.variant == variant)
        .expect("checked exhaustive match");
    let mut body = arm.body.clone();
    // All values have already been evaluated, including unbound default payloads.
    for (local, value) in arm.bindings.iter().zip(values).rev() {
        body = Expr {
            ty: body.ty.clone(),
            span: body.span,
            kind: ExprKind::Let {
                local: *local,
                value: Box::new(value.clone()),
                body: Box::new(body),
            },
        };
    }
    body
}

fn replace_consumers(e: &mut Expr, local: LocalId, variant: usize, values: &[Expr]) {
    if let ExprKind::Match { subject, arms, .. } = &e.kind
        && matches!(subject.kind, ExprKind::Local(id) if id == local)
    {
        *e = select(arms, variant, values);
    } else if let ExprKind::Drop(value) = &e.kind
        && matches!(value.kind, ExprKind::Local(id) if id == local)
    {
        e.kind = ExprKind::Nop;
    }
    for child in e.children_mut() {
        replace_consumers(child, local, variant, values);
    }
}
