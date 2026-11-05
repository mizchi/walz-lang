//! Source-observable effect contracts, separate from lowering/allocation facts.

use crate::Error;
use crate::ir::*;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct EffectSet {
    pub reads: bool,
    pub writes: bool,
    /// Creates an identity observable by the source, not merely a value box.
    pub allocates: bool,
    pub suspends: bool,
    pub host: bool,
    pub throws: BTreeSet<TagId>,
    pub throws_unknown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectContract {
    pub span: Span,
    pub allowed: EffectSet,
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectSummary {
    pub reads: bool,
    pub writes: bool,
    pub allocates: bool,
    pub suspends: bool,
    pub host: bool,
    pub throws: Vec<String>,
    pub throws_unknown: bool,
}

impl EffectSet {
    pub fn summary(&self, module: &Module) -> EffectSummary {
        EffectSummary {
            reads: self.reads,
            writes: self.writes,
            allocates: self.allocates,
            suspends: self.suspends,
            host: self.host,
            throws: self
                .throws
                .iter()
                .map(|id| module.tags[id.0].name.clone())
                .collect(),
            throws_unknown: self.throws_unknown,
        }
    }

    fn unknown() -> Self {
        Self {
            reads: true,
            writes: true,
            allocates: true,
            suspends: true,
            host: true,
            throws: BTreeSet::new(),
            throws_unknown: true,
        }
    }

    fn union(&mut self, other: &Self) {
        self.reads |= other.reads;
        self.writes |= other.writes;
        self.allocates |= other.allocates;
        self.suspends |= other.suspends;
        self.host |= other.host;
        self.throws.extend(&other.throws);
        self.throws_unknown |= other.throws_unknown;
    }
}

fn direct(expression: &Expr, module: &Module, functions: &[EffectSet]) -> EffectSet {
    use ExprKind::*;
    let mut effects = EffectSet::default();
    match &expression.kind {
        Call { function, .. } => effects.union(&functions[function.0]),
        Delivery { op, .. } => effects.suspends = op.suspends(),
        StructNew { .. } | ArrayNew { .. } | ArrayFixed { .. } => effects.allocates = true,
        VariantNew { ty, .. } => {
            effects.allocates = !matches!(module.types[ty.0].kind, TypeKind::Value(_))
        }
        StructGet { ty, field, .. } => {
            let TypeKind::Struct(fields) = &module.types[ty.0].kind else {
                unreachable!()
            };
            effects.reads = fields[*field].mutable;
        }
        Buffer {
            op: BufferOp::FromArray(ty),
            ..
        } => {
            let TypeKind::Array(field) = &module.types[ty.0].kind else {
                unreachable!()
            };
            effects.reads = field.mutable;
        }
        ArrayGet { ty, .. } => {
            let TypeKind::Array(field) = &module.types[ty.0].kind else {
                unreachable!()
            };
            effects.reads = field.mutable;
        }
        StructSet { .. } | ArraySet { .. } => effects.writes = true,
        Throw { tag, .. } => {
            effects.throws.insert(*tag);
        }
        ResourceNew { .. } => effects.allocates = true,
        ResourceHandle(_) => effects.reads = true,
        ResourceClose(v) | Using { value: v, .. } => {
            let Type::Resource(id) = v.ty else {
                unreachable!()
            };
            let TypeKind::Resource { close, .. } = module.types[id.0].kind else {
                unreachable!()
            };
            effects.union(&functions[close.0]);
            effects.writes = true;
        }
        _ => {}
    }
    effects
}

fn visit(expression: &Expr, module: &Module, functions: &[EffectSet]) -> EffectSet {
    let mut effects = direct(expression, module, functions);
    if let ExprKind::Try { body, handlers } = &expression.kind {
        let mut body = visit(body, module, functions);
        for handler in handlers {
            if let Some(tag) = handler.tag {
                body.throws.remove(&tag);
            } else {
                body.throws.clear();
                body.throws_unknown = false;
            }
        }
        effects.union(&body);
        for handler in handlers {
            effects.union(&visit(&handler.body, module, functions));
        }
        return effects;
    }
    for child in expression.children() {
        effects.union(&visit(child, module, functions));
    }
    effects
}

/// Infer effects from actual bodies, never from user-declared bounds. Unknown
/// imports remain top; recursion and caught tags are solved to a fixed point.
pub fn infer(module: &Module) -> Vec<EffectSet> {
    let mut effects = vec![EffectSet::default(); module.functions.len()];
    loop {
        let next = module
            .functions
            .iter()
            .map(|function| {
                function
                    .body
                    .as_ref()
                    .map(|body| visit(body, module, &effects))
                    .unwrap_or_else(|| {
                        crate::bindings::effects(function).unwrap_or_else(EffectSet::unknown)
                    })
            })
            .collect::<Vec<_>>();
        if next == effects {
            return effects;
        }
        effects = next;
    }
}

#[derive(Clone, Copy)]
enum Requirement {
    Reads,
    Writes,
    Allocates,
    Suspends,
    Host,
    Throws(TagId),
    ThrowsUnknown,
}
impl Requirement {
    fn present(self, effects: &EffectSet) -> bool {
        match self {
            Self::Reads => effects.reads,
            Self::Writes => effects.writes,
            Self::Allocates => effects.allocates,
            Self::Suspends => effects.suspends,
            Self::Host => effects.host,
            Self::Throws(tag) => effects.throws.contains(&tag),
            Self::ThrowsUnknown => effects.throws_unknown,
        }
    }
    fn name(self, module: &Module) -> String {
        match self {
            Self::Reads => "reads".into(),
            Self::Writes => "writes".into(),
            Self::Allocates => "allocates".into(),
            Self::Suspends => "suspends".into(),
            Self::Host => "host".into(),
            Self::ThrowsUnknown => "throws(*)".into(),
            Self::Throws(tag) => format!("throws({})", module.tags[tag.0].name),
        }
    }
    fn all(effects: &EffectSet) -> Vec<Self> {
        let mut kinds = vec![
            Self::Reads,
            Self::Writes,
            Self::Allocates,
            Self::Suspends,
            Self::Host,
            Self::ThrowsUnknown,
        ];
        kinds.extend(effects.throws.iter().copied().map(Self::Throws));
        kinds
            .into_iter()
            .filter(|kind| kind.present(effects))
            .collect()
    }
}
fn describe(effects: &EffectSet, module: &Module) -> String {
    let names = Requirement::all(effects)
        .iter()
        .map(|kind| kind.name(module))
        .collect::<Vec<_>>();
    if names.is_empty() {
        "pure".into()
    } else {
        names.join(", ")
    }
}

/// Find a witness using the same effect rules as inference. Calls retain their
/// chain, and caught exceptions are never reported as escaping a try. Recursion
/// is cut by function ID; a long chain still has a truthful call-site witness.
fn origin(
    expression: &Expr,
    requirement: Requirement,
    module: &Module,
    functions: &[EffectSet],
    seen: &mut BTreeSet<FunctionId>,
) -> Option<Vec<(Span, String)>> {
    if requirement.present(&direct(expression, module, functions)) {
        if let ExprKind::Call { function, .. } = &expression.kind {
            let callee = &module.functions[function.0];
            let site = (
                expression.span,
                format!(
                    "call to `{}` requires {}",
                    callee.name,
                    requirement.name(module)
                ),
            );
            if seen.len() >= 8 || callee.body.is_none() {
                return Some(vec![site]);
            }
            if seen.insert(*function) {
                let nested = origin(
                    callee.body.as_ref().unwrap(),
                    requirement,
                    module,
                    functions,
                    seen,
                );
                seen.remove(function);
                if let Some(nested) = nested {
                    let mut chain = vec![site];
                    chain.extend(nested);
                    return Some(chain);
                }
            }
        } else {
            let operation = match expression.kind {
                ExprKind::StructGet { .. } => "mutable struct.get",
                ExprKind::ArrayGet { .. } => "mutable array.get",
                ExprKind::Buffer { .. } => "bytes.from-array",
                ExprKind::StructSet { .. } => "struct.set",
                ExprKind::ArraySet { .. } => "array.set",
                ExprKind::StructNew { .. } => "struct.new",
                ExprKind::ArrayNew { .. } | ExprKind::ArrayFixed { .. } => "array construction",
                ExprKind::VariantNew { .. } => "variant.new",
                ExprKind::Delivery { .. } => "delivery operation",
                ExprKind::Throw { .. } => "throw",
                _ => "operation",
            };
            return Some(vec![(
                expression.span,
                format!("{} required by {operation}", requirement.name(module)),
            )]);
        }
    }
    let children = if let ExprKind::Try { body, handlers } = &expression.kind {
        let caught = match requirement {
            Requirement::Throws(tag) => handlers
                .iter()
                .any(|h| h.tag.is_none() || h.tag == Some(tag)),
            Requirement::ThrowsUnknown => handlers.iter().any(|h| h.tag.is_none()),
            _ => false,
        };
        std::iter::once(body.as_ref())
            .filter(|_| !caught)
            .chain(handlers.iter().map(|h| &h.body))
            .collect::<Vec<_>>()
    } else {
        expression.children()
    };
    for child in children {
        if requirement.present(&visit(child, module, functions))
            && let Some(origin) = origin(child, requirement, module, functions, seen)
        {
            return Some(origin);
        }
    }
    None
}

pub(crate) fn validate(source: &str, module: &Module) -> Result<(), Error> {
    if module.functions.iter().all(|f| f.effect_contract.is_none()) {
        return Ok(());
    }
    let inferred = infer(module);
    for (index, (function, actual)) in module.functions.iter().zip(&inferred).enumerate() {
        let Some(contract) = &function.effect_contract else {
            continue;
        };
        let missing = Requirement::all(actual)
            .into_iter()
            .filter(|kind| match kind {
                Requirement::Throws(_) | Requirement::ThrowsUnknown
                    if contract.allowed.throws_unknown =>
                {
                    false
                }
                _ => !kind.present(&contract.allowed),
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let mut error = Error::at(
                source,
                contract.span,
                format!(
                    "effect contract for `{}` exceeded: {}",
                    function.name,
                    missing
                        .iter()
                        .map(|kind| kind.name(module))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
            .with_code("E_EFFECT_CONTRACT")
            .note(format!(
                "declared: {}; required: {}",
                describe(&contract.allowed, module),
                describe(actual, module)
            ));
            if let Some(body) = &function.body {
                for requirement in missing {
                    if let Some(chain) = origin(
                        body,
                        requirement,
                        module,
                        &inferred,
                        &mut BTreeSet::from([FunctionId(index)]),
                    ) {
                        for (span, message) in chain {
                            error = error.related(source, span, message);
                        }
                    }
                }
            }
            return Err(error.help("remove the operations exceeding the declared bound, or explicitly allow their effects if intended"));
        }
    }
    Ok(())
}
