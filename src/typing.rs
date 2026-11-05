//! Source-level type information projected from the statically checked IR.

use crate::ir::{BindingKind, Expr, LocalId, Module, Span, Type};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TypeReport {
    pub functions: Vec<FunctionTypes>,
}

#[derive(Debug, Serialize)]
pub struct FunctionTypes {
    pub name: String,
    pub params: Vec<LocalId>,
    pub captures: Vec<LocalId>,
    pub result: String,
    pub declared_effects: Option<crate::effects::EffectSummary>,
    pub locals: Vec<LocalType>,
    pub expressions: Vec<ExpressionType>,
}

#[derive(Debug, Serialize)]
pub struct LocalType {
    /// Function-local ID; distinct even when source names shadow one another.
    pub id: LocalId,
    pub name: String,
    pub span: Span,
    pub kind: BindingKind,
    pub ty: String,
}

#[derive(Debug, Serialize)]
pub struct ExpressionType {
    /// Half-open UTF-8 byte range into the original source.
    pub span: Span,
    pub ty: String,
}

/// List every declared or inferred binding type and every source expression's type.
/// `module` must be the typed IR returned by [`crate::check`].
pub fn report(module: &Module) -> TypeReport {
    TypeReport {
        functions: module
            .functions
            .iter()
            .map(|function| {
                let mut expressions = Vec::new();
                let mut pending = function.body.iter().collect::<Vec<_>>();
                while let Some(expression) = pending.pop() {
                    if expression.span.start < expression.span.end {
                        expressions.push(ExpressionType {
                            span: expression.span,
                            ty: module.type_name(&expression.ty),
                        });
                    }
                    pending.extend(expression.children().into_iter().rev());
                }
                FunctionTypes {
                    name: function.name.clone(),
                    params: function.params.clone(),
                    captures: function.captures.clone(),
                    result: module.type_name(&function.result),
                    declared_effects: function
                        .effect_contract
                        .as_ref()
                        .map(|c| c.allowed.summary(module)),
                    locals: function
                        .locals
                        .iter()
                        .enumerate()
                        .map(|(index, local)| LocalType {
                            id: LocalId(index),
                            name: local.name.clone(),
                            span: local.span,
                            kind: local.kind.clone(),
                            ty: module.type_name(&local.ty),
                        })
                        .collect(),
                    expressions,
                }
            })
            .collect(),
    }
}

fn contains(expression: &Expr, offset: usize) -> bool {
    expression.span.start <= offset && offset < expression.span.end
}

/// Get a binding declaration's or the innermost expression's static type
/// at a UTF-8 byte offset. Returns `None` outside these source ranges.
/// `module` must be the typed IR returned by [`crate::check`].
pub fn type_at(module: &Module, offset: usize) -> Option<&Type> {
    if let Some(local) = module
        .functions
        .iter()
        .flat_map(|f| &f.locals)
        .find(|local| local.span.start <= offset && offset < local.span.end)
    {
        return Some(&local.ty);
    }
    let mut expression = module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .find(|body| contains(body, offset))?;
    while let Some(child) = expression
        .children()
        .into_iter()
        .find(|child| contains(child, offset))
    {
        expression = child;
    }
    Some(&expression.ty)
}
