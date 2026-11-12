//! Bounded expansion of shared expression helpers. Locals and loop labels are
//! fresh; arguments remain left-to-right lets. Return/branch/exception scopes,
//! resources, recursion, and large functions are left as calls.
use crate::ir::*;
use std::collections::BTreeMap;

pub(super) fn nodes(e: &Expr) -> Vec<&Expr> {
    let mut out = vec![e];
    let mut i = 0;
    while i < out.len() {
        out.extend(out[i].children());
        i += 1;
    }
    out
}

pub(super) fn shared(f: &Function, kinds: &crate::ownership::Kinds) -> bool {
    !kinds.is_owned(&f.result)
        && !matches!(f.result, Type::Borrow(_) | Type::Resource(_))
        && f.locals
            .iter()
            .all(|l| !kinds.is_owned(&l.ty) && !matches!(l.ty, Type::Borrow(_) | Type::Resource(_)))
}

#[derive(Clone, Copy)]
enum Purpose {
    Ranges,
    Values,
}

pub(super) fn private_value(module: &Module, ty: TypeId) -> bool {
    matches!(
        module.types[ty.0].kind,
        TypeKind::Value(ValueType::Pair { .. } | ValueType::Option(_) | ValueType::Result { .. })
    )
}

fn supported(e: &Expr, module: &Module, purpose: Purpose) -> bool {
    use ExprKind::*;
    match &e.kind {
        Local(_)
        | Const { .. }
        | Numeric { .. }
        | If { .. }
        | Let { .. }
        | Block(_)
        | Loop { .. }
        | Recur { .. }
        | Call { .. }
        | StructGet { .. }
        | ArrayLen(_)
        | ArrayGet { .. }
        | Drop(_)
        | Nop
        | Unreachable => true,
        StructNew { ty, .. } => {
            matches!(&module.types[ty.0].kind, TypeKind::Struct(fields) if fields.iter().all(|f| !f.mutable))
        }
        Buffer { op, .. } => !matches!(op, BufferOp::CopyToArray(_) | BufferOp::WriteArray(..)),
        VariantNew { ty, .. } | Match { ty, .. } => {
            matches!(purpose, Purpose::Values) && private_value(module, *ty)
        }
        _ => false,
    }
}

pub(super) struct Plan {
    candidates: Vec<bool>,
}
impl Plan {
    pub(super) fn new(module: &Module, kinds: &crate::ownership::Kinds) -> Self {
        Self::build(module, kinds, Purpose::Ranges)
    }

    pub(super) fn values(module: &Module, kinds: &crate::ownership::Kinds) -> Self {
        Self::build(module, kinds, Purpose::Values)
    }

    fn build(module: &Module, kinds: &crate::ownership::Kinds, purpose: Purpose) -> Self {
        let mut interesting = vec![false; module.functions.len()];
        let mut reverse = vec![vec![]; module.functions.len()];
        let mut eligible = vec![false; module.functions.len()];
        for (i, f) in module.functions.iter().enumerate() {
            let Some(body) = &f.body else { continue };
            let items = nodes(body);
            eligible[i] = matches!(f.kind, FunctionKind::Defined)
                && shared(f, kinds)
                && f.captures.is_empty()
                && f.locals.len() <= 160
                && items.len() <= 128
                && items.iter().all(|e| supported(e, module, purpose));
            if matches!(purpose, Purpose::Values)
                && let Type::Value(ty) = f.result
                && private_value(module, ty)
            {
                interesting[i] = true;
            }
            for e in items {
                if matches!(purpose, Purpose::Values)
                    && matches!(e.kind, ExprKind::VariantNew { ty, .. } | ExprKind::Match { ty, .. }
                        if private_value(module, ty))
                {
                    interesting[i] = true;
                }
                if matches!(purpose, Purpose::Ranges)
                    && matches!(
                        e.kind,
                        ExprKind::Buffer { .. }
                            | ExprKind::ArrayLen(_)
                            | ExprKind::StructGet { .. }
                    )
                {
                    interesting[i] = true;
                }
                if let ExprKind::Call { function, .. } = e.kind {
                    reverse[function.0].push(i);
                }
            }
        }
        let mut pending = interesting
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.then_some(i))
            .collect::<Vec<_>>();
        while let Some(i) = pending.pop() {
            for &caller in &reverse[i] {
                if !interesting[caller] {
                    interesting[caller] = true;
                    pending.push(caller);
                }
            }
        }
        Self {
            candidates: eligible
                .into_iter()
                .zip(interesting)
                .map(|(a, b)| a && b)
                .collect(),
        }
    }

    pub(super) fn expand(&self, module: &Module, index: usize) -> Function {
        let mut f = module.functions[index].clone();
        let Some(mut body) = f.body.take() else {
            return f;
        };
        let next_loop = nodes(&body)
            .iter()
            .filter_map(|e| match e.kind {
                ExprKind::Loop { label, .. } | ExprKind::Label { label, .. } => Some(label.0 + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let mut visitor = Expand {
            module,
            plan: self,
            locals: &mut f.locals,
            next_loop,
            budget: 1500,
            stack: vec![index],
        };
        visitor.walk(&mut body, 0);
        f.body = Some(body);
        f
    }
}

struct Expand<'a> {
    module: &'a Module,
    plan: &'a Plan,
    locals: &'a mut Vec<Binding>,
    next_loop: usize,
    budget: usize,
    stack: Vec<usize>,
}
impl Expand<'_> {
    fn walk(&mut self, e: &mut Expr, depth: usize) {
        if let ExprKind::Call { function, args } = &e.kind {
            let f = &self.module.functions[function.0];
            if self.plan.candidates[function.0] && depth < 8 && !self.stack.contains(&function.0) {
                let source = f.body.as_ref().unwrap();
                let items = nodes(source);
                let cost = items.len() + f.locals.len();
                if cost <= self.budget {
                    self.budget -= cost;
                    let id = function.0;
                    let mut labels = BTreeMap::new();
                    for item in items {
                        if let ExprKind::Loop { label, .. } = item.kind {
                            labels.insert(label, LoopId(self.next_loop));
                            self.next_loop += 1;
                        }
                    }
                    let offset = self.locals.len();
                    for binding in &f.locals {
                        let mut binding = binding.clone();
                        binding.name = format!("$__inline_{}", self.locals.len());
                        binding.span = e.span;
                        binding.kind = match binding.kind {
                            BindingKind::Loop(label) => BindingKind::Loop(labels[&label]),
                            _ => BindingKind::Let,
                        };
                        self.locals.push(binding);
                    }
                    let mut body = source.clone();
                    remap(&mut body, offset, &labels);
                    for (param, value) in f.params.iter().zip(args).rev() {
                        body = Expr {
                            ty: body.ty.clone(),
                            span: e.span,
                            kind: ExprKind::Let {
                                local: LocalId(offset + param.0),
                                value: Box::new(value.clone()),
                                body: Box::new(body),
                            },
                        };
                    }
                    *e = body;
                    self.stack.push(id);
                    self.walk(e, depth + 1);
                    self.stack.pop();
                    return;
                }
            }
        }
        for child in e.children_mut() {
            self.walk(child, depth);
        }
    }
}

fn remap(e: &mut Expr, offset: usize, labels: &BTreeMap<LoopId, LoopId>) {
    match &mut e.kind {
        ExprKind::Local(id) | ExprKind::Let { local: id, .. } => id.0 += offset,
        ExprKind::Loop {
            label, bindings, ..
        } => {
            *label = labels[label];
            for (id, _) in bindings {
                id.0 += offset;
            }
        }
        ExprKind::Recur { label, assignments } => {
            *label = labels[label];
            for (id, _) in assignments {
                id.0 += offset;
            }
        }
        ExprKind::Match { arms, .. } => {
            for arm in arms {
                for id in &mut arm.bindings {
                    id.0 += offset;
                }
            }
        }
        _ => {}
    }
    for child in e.children_mut() {
        remap(child, offset, labels);
    }
}
