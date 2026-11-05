//! Explicit ownership contracts over the typed core. Shared GC references keep
//! their existing semantics. Owned boxes, delivery endpoints, aggregates, and
//! their capturing closures move.

use crate::Error;
use crate::ir::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

type Context = BTreeSet<LocalId>;

/// The small ownership language independently checked by proof/Ownership.lean.
#[derive(Clone, Debug, Serialize)]
pub struct Contract {
    pub span: Span,
    #[serde(flatten)]
    pub kind: Operation,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Nop,
    Take {
        id: LocalId,
    },
    Scope {
        id: LocalId,
        body: Box<Contract>,
    },
    Seq {
        items: Vec<Contract>,
    },
    Branch {
        arms: Vec<Contract>,
    },
    Loop {
        owners: Vec<LocalId>,
        body: Box<Contract>,
    },
    Again,
    Stop,
}

#[derive(Debug, Serialize)]
pub struct FunctionContract {
    pub name: String,
    pub inputs: Vec<LocalId>,
    pub body: Contract,
}

#[derive(Debug, Serialize)]
pub struct Contracts {
    pub version: u32,
    pub functions: Vec<FunctionContract>,
}

/// Ownership is structural for value aggregates and nominal closure captures.
/// Compute the least fixed point to support forward and recursive closure types.
pub struct Kinds {
    owned: Vec<bool>,
}

impl Kinds {
    pub fn new(module: &Module) -> Self {
        let mut kinds = Self {
            owned: vec![false; module.types.len()],
        };
        loop {
            let next = module
                .types
                .iter()
                .enumerate()
                .map(|(index, declaration)| match &declaration.kind {
                    TypeKind::Value(
                        ValueType::Own(_) | ValueType::Future(_) | ValueType::Stream(_),
                    ) => true,
                    TypeKind::Value(value) => value
                        .variants(TypeId(index))
                        .iter()
                        .flat_map(|v| &v.payloads)
                        .any(|ty| kinds.is_owned(ty)),
                    TypeKind::Closure { captures, .. } => {
                        captures.iter().any(|f| kinds.is_owned(&f.ty))
                    }
                    _ => false,
                })
                .collect::<Vec<_>>();
            if kinds.owned == next {
                return kinds;
            }
            kinds.owned = next;
        }
    }

    pub fn is_owned(&self, ty: &Type) -> bool {
        match ty {
            Type::Value(id) | Type::Ref { ty: id, .. } => self.owned[id.0],
            _ => false,
        }
    }

    fn check_type(&self, source: &str, ty: &Type, span: Span) -> Result<(), Error> {
        if self.is_owned(ty) && matches!(ty, Type::Ref { nullable: true, .. }) {
            return Err(Error::at(source, span, "owned closures cannot be nullable"));
        }
        Ok(())
    }
}

fn node(span: Span, kind: Operation) -> Contract {
    Contract { span, kind }
}

struct Extractor<'a> {
    source: &'a str,
    function: &'a Function,
    kinds: &'a Kinds,
}

impl Extractor<'_> {
    fn scoped(&self, locals: impl IntoIterator<Item = LocalId>, body: Contract) -> Contract {
        locals
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .fold(body, |body, id| {
                if self.kinds.is_owned(&self.function.locals[id.0].ty) {
                    node(
                        self.function.locals[id.0].span,
                        Operation::Scope {
                            id,
                            body: Box::new(body),
                        },
                    )
                } else {
                    body
                }
            })
    }

    fn expression(&self, expression: &Expr) -> Result<Contract, Error> {
        use ExprKind::*;
        let span = expression.span;
        self.kinds.check_type(self.source, &expression.ty, span)?;
        let kind = match &expression.kind {
            Local(id) if self.kinds.is_owned(&expression.ty) => {
                return Err(Error::at(
                    self.source,
                    span,
                    format!(
                        "owned local `{}` requires explicit move",
                        self.function.locals[id.0].name
                    ),
                ));
            }
            Move(id) => {
                if !self.kinds.is_owned(&expression.ty) {
                    return Err(Error::at(self.source, span, "move requires an owned value"));
                }
                Operation::Take { id: *id }
            }
            Drop(value) if self.kinds.is_owned(&value.ty) => {
                return Err(Error::at(
                    self.source,
                    span,
                    "cannot drop an owned value; consume, return, unpack, or own.close it",
                ));
            }
            RefEq(a, b) if self.kinds.is_owned(&a.ty) || self.kinds.is_owned(&b.ty) => {
                return Err(Error::at(
                    self.source,
                    span,
                    "owned closure identity cannot be inspected",
                ));
            }
            IsNull(value) | AsNonNull(value) if self.kinds.is_owned(&value.ty) => {
                return Err(Error::at(
                    self.source,
                    span,
                    "owned closure references cannot be inspected",
                ));
            }
            Let { local, value, body } => Operation::Seq {
                items: vec![
                    self.expression(value)?,
                    self.scoped([*local], self.expression(body)?),
                ],
            },
            If {
                condition,
                then_body,
                else_body,
            } => Operation::Seq {
                items: vec![
                    self.expression(condition)?,
                    node(
                        span,
                        Operation::Branch {
                            arms: vec![self.expression(then_body)?, self.expression(else_body)?],
                        },
                    ),
                ],
            },
            Match { subject, arms, .. } => Operation::Seq {
                items: vec![
                    self.expression(subject)?,
                    node(
                        span,
                        Operation::Branch {
                            arms: arms
                                .iter()
                                .map(|arm| {
                                    Ok(self.scoped(
                                        arm.bindings.iter().copied(),
                                        self.expression(&arm.body)?,
                                    ))
                                })
                                .collect::<Result<_, Error>>()?,
                        },
                    ),
                ],
            },
            Loop { bindings, body, .. } => {
                let mut items = bindings
                    .iter()
                    .map(|(_, value)| self.expression(value))
                    .collect::<Result<Vec<_>, _>>()?;
                items.push(node(
                    span,
                    Operation::Loop {
                        owners: bindings
                            .iter()
                            .filter(|(id, _)| self.kinds.is_owned(&self.function.locals[id.0].ty))
                            .map(|(id, _)| *id)
                            .collect(),
                        body: Box::new(self.expression(body)?),
                    },
                ));
                Operation::Seq { items }
            }
            Recur { assignments, .. } => {
                let mut items = assignments
                    .iter()
                    .map(|(_, value)| self.expression(value))
                    .collect::<Result<Vec<_>, _>>()?;
                items.push(node(span, Operation::Again));
                Operation::Seq { items }
            }
            Unreachable | Throw { .. } => {
                let mut items = expression
                    .children()
                    .into_iter()
                    .map(|e| self.expression(e))
                    .collect::<Result<Vec<_>, _>>()?;
                items.push(node(span, Operation::Stop));
                Operation::Seq { items }
            }
            // Owned functions reject exceptional control before extraction.
            // With no owners, try/catch has no ownership facts to transport.
            Try { .. } => {
                for child in expression.children() {
                    self.expression(child)?;
                }
                Operation::Nop
            }
            _ => Operation::Seq {
                items: expression
                    .children()
                    .into_iter()
                    .map(|e| self.expression(e))
                    .collect::<Result<_, _>>()?,
            },
        };
        Ok(node(span, kind))
    }
}

/// Diagnostic provenance is deliberately outside the serialized proof contract.
/// Only `live` participates in ownership rules and loop/branch equality.
#[derive(Clone)]
struct State {
    live: Context,
    moved: BTreeMap<LocalId, BTreeSet<Span>>,
}

/// None is an abrupt path (recur or stop). Only reachable normal paths
/// participate in branch joins, including the provenance of previous moves.
fn check_contract(
    source: &str,
    function: &Function,
    contract: &Contract,
    mut state: State,
    loops: &[Context],
) -> Result<Option<State>, Error> {
    let fail = |message: &str| Error::at(source, contract.span, message).with_code("E_OWNERSHIP");
    let binding = |error: Error, id: LocalId| {
        let local = &function.locals[id.0];
        error.related(source, local.span, format!("`{}` bound here", local.name))
    };
    Ok(match &contract.kind {
        Operation::Nop => Some(state),
        Operation::Take { id } => {
            if !state.live.remove(id) {
                let mut error = binding(
                    fail(&format!(
                        "owned value `{}` is already moved or unavailable",
                        function.locals[id.0].name
                    ))
                    .with_code("E_OWNERSHIP_MOVED"),
                    *id,
                );
                if let Some(origins) = state.moved.get(id) {
                    for span in origins.iter().take(3) {
                        error = error.related(
                            source,
                            *span,
                            if origins.len() == 1 {
                                "value first moved here"
                            } else {
                                "value moved here on a reachable branch"
                            },
                        );
                    }
                    if origins.len() > 3 {
                        error = error.note("additional branch move locations omitted");
                    }
                }
                return Err(error);
            }
            state.moved.insert(*id, BTreeSet::from([contract.span]));
            Some(state)
        }
        Operation::Scope { id, body } => {
            if !state.live.insert(*id) {
                return Err(binding(fail("owned binding is already live"), *id));
            }
            state.moved.remove(id);
            let result = check_contract(source, function, body, state, loops)?;
            if result.as_ref().is_some_and(|state| state.live.contains(id)) {
                return Err(binding(fail("unconsumed owned binding at scope exit"), *id));
            }
            result
        }
        Operation::Seq { items } => {
            let mut result = Some(state);
            for item in items {
                if let Some(state) = result {
                    result = check_contract(source, function, item, state, loops)?;
                }
            }
            result
        }
        Operation::Branch { arms } => {
            if arms.is_empty() {
                return Err(fail("empty ownership branch"));
            }
            let mut output: Option<State> = None;
            for arm in arms {
                if let Some(result) = check_contract(source, function, arm, state.clone(), loops)? {
                    if let Some(previous) = &mut output {
                        if previous.live != result.live {
                            return Err(fail("branch ownership states differ"));
                        }
                        for (id, spans) in result.moved {
                            previous.moved.entry(id).or_default().extend(spans);
                        }
                    } else {
                        output = Some(result);
                    }
                }
            }
            output
        }
        Operation::Loop { owners, body } => {
            let mut nested = loops.to_vec();
            nested.push(state.live.clone());
            for id in owners {
                if !state.live.insert(*id) {
                    return Err(binding(fail("loop owner is already live"), *id));
                }
                state.moved.remove(id);
            }
            let result = check_contract(source, function, body, state, &nested)?;
            if let Some(state) = &result
                && let Some(id) = owners.iter().find(|id| state.live.contains(id))
            {
                return Err(binding(fail("unconsumed owned loop binding at exit"), *id));
            }
            result
        }
        Operation::Again => {
            if loops.last() != Some(&state.live) {
                return Err(fail("loop ownership contract is not restored before recur"));
            }
            None
        }
        Operation::Stop => {
            if let Some(id) = state.live.first() {
                return Err(binding(fail("unconsumed owned value on abrupt exit"), *id));
            }
            None
        }
    })
}

fn has_owned_expression(expression: &Expr, kinds: &Kinds) -> bool {
    kinds.is_owned(&expression.ty)
        || expression
            .children()
            .into_iter()
            .any(|e| has_owned_expression(e, kinds))
}

fn has_exceptional_control(expression: &Expr) -> bool {
    matches!(
        expression.kind,
        ExprKind::Try { .. } | ExprKind::Throw { .. }
    ) || expression
        .children()
        .into_iter()
        .any(has_exceptional_control)
}

/// Extract and check ownership obligations. This is also the contract sent to
/// the independent Lean checker by the verified execution path.
pub fn validate(source: &str, module: &Module) -> Result<Contracts, Error> {
    let kinds = Kinds::new(module);
    let origin = Span::default();
    for declaration in &module.types {
        let fields = match &declaration.kind {
            TypeKind::Struct(fields) => fields.iter().map(|f| &f.ty).collect::<Vec<_>>(),
            TypeKind::Array(field) => vec![&field.ty],
            TypeKind::Variant(variants) => variants.iter().flat_map(|v| &v.payloads).collect(),
            TypeKind::Value(ValueType::Own(ty)) if kinds.is_owned(ty) => {
                return Err(Error::at(source, origin, "own payload must be shared data"));
            }
            TypeKind::Value(ValueType::Future(ty) | ValueType::Stream(ty))
                if kinds.is_owned(ty) =>
            {
                return Err(Error::at(
                    source,
                    origin,
                    "delivery payload must be shared data in this version",
                ));
            }
            _ => vec![],
        };
        if fields.iter().any(|ty| kinds.is_owned(ty)) {
            return Err(Error::at(
                source,
                origin,
                format!(
                    "owned values cannot be stored in shared GC storage `{}`",
                    declaration.name
                ),
            ));
        }
    }
    if module
        .tags
        .iter()
        .any(|tag| tag.payloads.iter().any(|ty| kinds.is_owned(ty)))
    {
        return Err(Error::at(
            source,
            origin,
            "owned values cannot be an exception payload",
        ));
    }
    let effects = crate::effects::infer(module);
    let mut functions = Vec::new();
    for (index, function) in module.functions.iter().enumerate() {
        for local in &function.locals {
            kinds.check_type(source, &local.ty, local.span)?;
        }
        kinds.check_type(source, &function.result, origin)?;
        let owns = function.locals.iter().any(|b| kinds.is_owned(&b.ty))
            || kinds.is_owned(&function.result)
            || function
                .body
                .as_ref()
                .is_some_and(|body| has_owned_expression(body, &kinds));
        let Some(body) = &function.body else {
            if owns && crate::bindings::lookup(function).is_none() {
                return Err(Error::at(
                    source,
                    origin,
                    "owned import requires a trusted resource binding; unavailable in this version",
                ));
            }
            continue;
        };
        if owns
            && (has_exceptional_control(body)
                || effects[index].throws_unknown
                || !effects[index].throws.is_empty())
        {
            return Err(Error::at(
                source,
                body.span,
                "owned functions cannot use exceptional control; return Result instead",
            ));
        }
        let extractor = Extractor {
            source,
            function,
            kinds: &kinds,
        };
        let contract = extractor.expression(body)?;
        let inputs = function
            .params
            .iter()
            .chain(&function.captures)
            .filter(|id| kinds.is_owned(&function.locals[id.0].ty))
            .copied()
            .collect::<Vec<_>>();
        let result = check_contract(
            source,
            function,
            &contract,
            State {
                live: inputs.iter().copied().collect(),
                moved: BTreeMap::new(),
            },
            &[],
        )?;
        if result.is_some_and(|state| !state.live.is_empty()) {
            return Err(Error::at(
                source,
                body.span,
                format!(
                    "unconsumed owned parameter or capture in `{}`",
                    function.name
                ),
            ));
        }
        functions.push(FunctionContract {
            name: function.name.clone(),
            inputs,
            body: contract,
        });
    }
    Ok(Contracts {
        version: 1,
        functions,
    })
}
