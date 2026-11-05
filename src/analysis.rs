//! Conservative effects and a complete call graph for the checked core.

use crate::ir::*;
use crate::{Analysis, Effects, FunctionSummary};
use std::collections::BTreeSet;

impl Effects {
    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            reads: self.reads || other.reads,
            writes: self.writes || other.writes,
            allocates: self.allocates || other.allocates,
            may_trap: self.may_trap || other.may_trap,
            may_throw: self.may_throw || other.may_throw,
            may_suspend: self.may_suspend || other.may_suspend,
            may_diverge: self.may_diverge || other.may_diverge,
            host: self.host || other.host,
        }
    }

    pub(crate) fn unknown() -> Self {
        Self {
            reads: true,
            writes: true,
            allocates: true,
            may_trap: true,
            may_throw: true,
            may_suspend: true,
            may_diverge: true,
            host: true,
        }
    }
}

fn nullable(expression: &Expr) -> bool {
    matches!(expression.ty, Type::Ref { nullable: true, .. })
}

fn visit(expression: &Expr, module: &Module, calls: &mut BTreeSet<FunctionId>) -> Effects {
    use ExprKind::*;
    let mut effects = Effects::default();
    match &expression.kind {
        Data(_) => {
            effects.allocates = true;
            effects.may_trap = true;
        }
        List {
            op: ListOp::FromArray(ty),
            ..
        }
        | Buffer {
            op: BufferOp::FromArray(ty),
            ..
        } => {
            let TypeKind::Array(field) = &module.types[ty.0].kind else {
                unreachable!()
            };
            effects.reads = field.mutable;
            effects.allocates = true;
            effects.may_trap = true;
        }
        List { op, .. } => {
            effects.allocates = matches!(op, ListOp::New | ListOp::Slice);
            effects.may_trap = !matches!(op, ListOp::Len);
        }
        Buffer { op, .. } => {
            effects.allocates = op.allocates();
            effects.may_trap = op.may_trap();
        }
        Numeric { op, .. } => effects.may_trap = op.signature().may_trap,
        Delivery { op, .. } => {
            effects.may_suspend = op.suspends();
            effects.allocates = op.allocates();
            effects.may_trap = op.may_trap();
        }
        ResourceNew { .. } => {
            effects.allocates = true;
            effects.may_trap = true;
        }
        ResourceHandle(_) => {
            effects.reads = true;
            effects.may_trap = true;
        }
        ResourceClose(v) | Using { value: v, .. } => {
            let Type::Resource(id) = v.ty else {
                unreachable!()
            };
            let TypeKind::Resource { close, .. } = module.types[id.0].kind else {
                unreachable!()
            };
            calls.insert(close);
            effects.writes = true;
            effects.may_trap = true;
        }
        Call { function, .. } => {
            calls.insert(*function);
        }
        Throw { .. } => {
            effects.may_throw = true;
            effects.allocates = true; // The engine creates a native exception object.
            effects.may_trap = true; // Includes resource exhaustion during creation.
        }
        StructNew { .. } | VariantNew { .. } | ArrayNew { .. } | ArrayFixed { .. } => {
            effects.allocates = true;
            effects.may_trap = true; // Includes allocation failure.
        }
        StructGet { ty, field, object } => {
            let TypeKind::Struct(fields) = &module.types[ty.0].kind else {
                unreachable!()
            };
            effects.reads = fields[*field].mutable;
            effects.may_trap = nullable(object);
        }
        StructSet { object, .. } => {
            effects.writes = true;
            effects.may_trap = nullable(object);
        }
        ArrayGet { ty, .. } => {
            let TypeKind::Array(field) = &module.types[ty.0].kind else {
                unreachable!()
            };
            effects.reads = field.mutable;
            effects.may_trap = true; // Bounds checks, even for non-null arrays.
        }
        ArraySet { .. } => {
            effects.writes = true;
            effects.may_trap = true;
        }
        ArrayLen(object) | AsNonNull(object) => effects.may_trap = nullable(object),
        Loop { .. } => effects.may_diverge = true,
        // Foreign Wasm can construct the open representation's base type or
        // forge its tag. Keep the defensive trap visible at ABI boundaries.
        Match { .. } => effects.may_trap = true,
        Unreachable => effects.may_trap = true,
        _ => {}
    }
    for child in expression.children() {
        effects = effects.union(visit(child, module, calls));
    }
    effects
}

/// Analyze a checked module. Effects are transitive; `calls` contains direct edges.
pub fn analyze(module: &Module) -> Analysis {
    let semantic = crate::effects::infer(module);
    let memory = crate::regions::analyze(module);
    let mut calls = Vec::new();
    let mut effects = Vec::new();
    for function in &module.functions {
        let mut edges = BTreeSet::new();
        let own = if let Some(body) = &function.body {
            visit(body, module, &mut edges)
        } else {
            if let Some(binding) = crate::bindings::lookup(function) {
                Effects {
                    host: true,
                    may_suspend: binding.suspends,
                    may_trap: true,
                    ..Effects::default()
                }
            } else {
                Effects::unknown()
            }
        };
        calls.push(edges);
        effects.push(own);
    }
    // Mark every member of a recursive cycle. Callers inherit this below.
    for (start, effect) in effects.iter_mut().enumerate() {
        let mut stack = calls[start].iter().copied().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        while let Some(target) = stack.pop() {
            if target.0 == start {
                effect.may_diverge = true;
                break;
            }
            if seen.insert(target) {
                stack.extend(calls[target.0].iter().copied());
            }
        }
    }
    loop {
        let previous = effects.clone();
        for (index, edges) in calls.iter().enumerate() {
            for target in edges {
                effects[index] = effects[index].union(previous[target.0]);
            }
        }
        if previous == effects {
            break;
        }
    }
    Analysis {
        opaque: false,
        module_effects: effects
            .iter()
            .fold(Effects::default(), |all, own| all.union(*own)),
        functions: module
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| FunctionSummary {
                name: function.name.clone(),
                captures: function
                    .captures
                    .iter()
                    .map(|id| function.locals[id.0].name.clone())
                    .collect(),
                calls: calls[index]
                    .iter()
                    .map(|id| module.functions[id.0].name.clone())
                    .collect(),
                effects: effects[index],
                semantic_effects: semantic[index].summary(module),
                declared_effects: function
                    .effect_contract
                    .as_ref()
                    .map(|c| c.allowed.summary(module)),
                memory: memory[index].clone(),
                facts: crate::flow::analyze(module, function),
            })
            .collect(),
    }
}
