//! Generic constraints are compile-time obligations over concrete types.
use crate::{
    Error,
    effects::{self, EffectContract},
    ir::*,
    ownership::Kinds,
    sexpr::Sexp,
    typeclasses::Class,
};
use std::collections::BTreeSet;

pub(crate) struct Site {
    pub generic: String,
    pub request: Span,
    pub clause: Sexp,
}

enum Clause<'a> {
    Shared(&'a [Sexp]),
    Nonunit(&'a [Sexp]),
    Class {
        class: Class,
        targets: &'a [Sexp],
    },
    Callable {
        target: &'a Sexp,
        params: &'a [Sexp],
        result: &'a Sexp,
        effects: Option<&'a Sexp>,
    },
}

fn parse<'a>(source: &str, node: &'a Sexp) -> Result<Clause<'a>, Error> {
    let fail = || {
        Error::at(source, node.span, "expected shared/nonunit or built-in type-class constraints, or (callable $F (param types...) (result type) [(%effect ...)])").with_code("E_GENERIC_CONSTRAINT")
    };
    let items = node.list().ok_or_else(fail)?;
    Ok(match node.head() {
        Some("shared") if items.len() >= 2 => Clause::Shared(&items[1..]),
        Some("nonunit") if items.len() >= 2 => Clause::Nonunit(&items[1..]),
        Some(name) if items.len() >= 2 && Class::parse(name).is_some() => Clause::Class {
            class: Class::parse(name).unwrap(),
            targets: &items[1..],
        },
        Some("callable") if (4..=5).contains(&items.len()) => {
            let params = items[2]
                .list()
                .filter(|_| items[2].head() == Some("param"))
                .ok_or_else(fail)?;
            let result = items[3]
                .list()
                .filter(|p| items[3].head() == Some("result") && p.len() == 2)
                .ok_or_else(fail)?;
            let effects = items.get(4);
            if let Some(effects) = effects {
                if effects.head() != Some("effect") {
                    return Err(fail());
                }
                crate::frontend::validate_effect_syntax(source, effects)?;
            }
            Clause::Callable {
                target: &items[1],
                params: &params[1..],
                result: &result[1],
                effects,
            }
        }
        _ => return Err(fail()),
    })
}

/// Validate even unapplied declarations, without assuming an abstract type checker.
pub(crate) fn validate_declaration(
    source: &str,
    node: &Sexp,
    parameters: &[String],
) -> Result<(), Error> {
    let Some(items) = node.list() else {
        return Ok(());
    };
    let groups = items
        .iter()
        .enumerate()
        .filter(|(_, n)| n.head() == Some("where"))
        .collect::<Vec<_>>();
    if groups.is_empty() {
        return Ok(());
    }
    if parameters.is_empty() || groups.len() != 1 || groups[0].0 != 3 {
        return Err(Error::at(
            source,
            groups[0].1.span,
            "where belongs immediately after type.params",
        )
        .with_code("E_GENERIC_CONSTRAINT"));
    }
    let group = groups[0].1;
    let clauses = &group.list().unwrap()[1..];
    if clauses.is_empty() {
        return Err(Error::at(
            source,
            group.span,
            "where requires at least one constraint",
        ));
    }
    for node in clauses {
        let targets = match parse(source, node)? {
            Clause::Shared(targets) | Clause::Nonunit(targets) | Clause::Class { targets, .. } => {
                targets
            }
            Clause::Callable { target, .. } => std::slice::from_ref(target),
        };
        for target in targets {
            if !target
                .atom()
                .is_some_and(|name| parameters.iter().any(|p| p == name))
            {
                return Err(Error::at(
                    source,
                    target.span,
                    "constraint target must be a declared type parameter",
                )
                .with_code("E_GENERIC_CONSTRAINT"));
            }
        }
    }
    Ok(())
}

impl Site {
    /// Reject categories already known from normalized arguments, before a
    /// substituted header can produce a less useful internal type error.
    pub fn check_categories(&self, source: &str) -> Result<(), Error> {
        for node in &self.clause.list().unwrap()[1..] {
            match parse(source, node)? {
                Clause::Nonunit(targets) if targets.iter().any(|t| t.atom() == Some("unit")) => {
                    return Err(self.error(source, node.span, "nonunit excludes unit"));
                }
                Clause::Class { class, targets } => {
                    for target in targets {
                        let ty = target.atom().and_then(NumType::parse).map(Type::Num);
                        if !ty.as_ref().is_some_and(|ty| class.supports(ty)) {
                            return Err(self.error(
                                source,
                                node.span,
                                format!(
                                    "{} has no instance for {}",
                                    class.name(),
                                    target.atom().unwrap_or("this type")
                                ),
                            ));
                        }
                    }
                }
                Clause::Callable { target, .. }
                    if !target.atom().is_some_and(|t| t.starts_with('$')) =>
                {
                    return Err(self.error(
                        source,
                        node.span,
                        "callable requires a nominal closure type",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn map_types(
        &mut self,
        mut f: impl FnMut(&Sexp) -> Result<Sexp, Error>,
    ) -> Result<(), Error> {
        crate::type_syntax::map_types(&mut self.clause, &mut |ty| {
            *ty = f(ty)?;
            Ok(())
        })
    }
    fn error(&self, source: &str, constraint: Span, message: impl std::fmt::Display) -> Error {
        Error::at(
            source,
            self.request,
            format!("generic constraint for {} failed: {message}", self.generic),
        )
        .with_code("E_GENERIC_BOUND")
        .related(source, constraint, "constraint declared here")
    }
}

#[derive(Clone)]
pub(crate) enum Argument {
    Value(Type),
    Heap(TypeId),
}

pub(crate) struct TypedClause<'a> {
    site: &'a Site,
    span: Span,
    kind: Bound,
}
enum Bound {
    Shared(Vec<Argument>),
    Nonunit(Vec<Argument>),
    Class {
        class: Class,
        targets: Vec<Argument>,
    },
    Callable {
        target: Argument,
        params: Vec<Type>,
        result: Type,
        effects: Option<EffectContract>,
    },
}
pub(crate) struct EffectObligation<'a> {
    site: &'a Site,
    span: Span,
    function: FunctionId,
    contract: EffectContract,
}

pub(crate) fn resolve<'a>(
    source: &str,
    sites: &'a [Site],
    mut argument: impl FnMut(&Sexp) -> Result<Argument, Error>,
    mut ty: impl FnMut(&Sexp) -> Result<Type, Error>,
    mut effect: impl FnMut(&Sexp) -> Result<EffectContract, Error>,
) -> Result<Vec<TypedClause<'a>>, Error> {
    let mut out = vec![];
    for site in sites {
        for node in &site.clause.list().unwrap()[1..] {
            let kind = match parse(source, node)? {
                Clause::Shared(types) => {
                    Bound::Shared(types.iter().map(&mut argument).collect::<Result<_, _>>()?)
                }
                Clause::Nonunit(types) => {
                    Bound::Nonunit(types.iter().map(&mut argument).collect::<Result<_, _>>()?)
                }
                Clause::Class { class, targets } => Bound::Class {
                    class,
                    targets: targets
                        .iter()
                        .map(&mut argument)
                        .collect::<Result<_, _>>()?,
                },
                Clause::Callable {
                    target,
                    params,
                    result,
                    effects,
                } => Bound::Callable {
                    target: argument(target)?,
                    params: params.iter().map(&mut ty).collect::<Result<_, _>>()?,
                    result: ty(result)?,
                    effects: effects.map(&mut effect).transpose()?,
                },
            };
            out.push(TypedClause {
                site,
                span: node.span,
                kind,
            });
        }
    }
    Ok(out)
}

fn shared(module: &Module, kinds: &Kinds, start: &Type) -> bool {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if kinds.is_owned(ty) || matches!(ty, Type::Borrow(_)) {
            return false;
        }
        let id = match ty {
            Type::Value(id) | Type::Ref { ty: id, .. } => *id,
            _ => continue,
        };
        if !seen.insert(id) {
            continue;
        }
        match &module.types[id.0].kind {
            TypeKind::Newtype(_) => {}
            TypeKind::Resource { .. } => return false,
            TypeKind::Struct(fields)
            | TypeKind::Closure {
                captures: fields, ..
            } => pending.extend(fields.iter().map(|f| &f.ty)),
            TypeKind::Array(field) => pending.push(&field.ty),
            TypeKind::Variant(variants) => {
                pending.extend(variants.iter().flat_map(|v| &v.payloads))
            }
            TypeKind::Value(value) => match value {
                ValueType::Bytes | ValueType::Text => {}
                ValueType::List(t)
                | ValueType::Option(t)
                | ValueType::Own(t)
                | ValueType::Future(t)
                | ValueType::Stream(t) => pending.push(t),
                ValueType::Pair { first, second } => pending.extend([first, second]),
                ValueType::Result { ok, err } => pending.extend([ok, err]),
            },
        }
    }
    true
}

fn argument_shared(module: &Module, kinds: &Kinds, argument: &Argument) -> bool {
    match argument {
        Argument::Value(t) => shared(module, kinds, t),
        Argument::Heap(id) => shared(
            module,
            kinds,
            &Type::Ref {
                ty: *id,
                nullable: false,
            },
        ),
    }
}

fn type_name(module: &Module, ty: &Type) -> String {
    if let Type::Value(id) = ty
        && let TypeKind::Value(value) = &module.types[id.0].kind
    {
        return value.type_name(|ty| type_name(module, ty));
    }
    ty.format_with(|id| module.types[id.0].name.clone())
}

fn signature(module: &Module, params: &[Type], result: &Type) -> String {
    format!(
        "({}) -> {}",
        params
            .iter()
            .map(|t| type_name(module, t))
            .collect::<Vec<_>>()
            .join(", "),
        type_name(module, result)
    )
}

pub(crate) fn check_shapes<'a>(
    source: &str,
    module: &Module,
    clauses: Vec<TypedClause<'a>>,
) -> Result<Vec<EffectObligation<'a>>, Error> {
    if clauses.is_empty() {
        return Ok(vec![]);
    }
    let kinds = Kinds::new(module);
    let mut effects = vec![];
    for TypedClause { site, span, kind } in clauses {
        let fail = |message: &str| site.error(source, span, message);
        match kind {
            Bound::Shared(targets) => {
                if targets.iter().any(|t| !argument_shared(module, &kinds, t)) {
                    return Err(fail("shared requires data without owned values or borrows"));
                }
            }
            Bound::Nonunit(targets) => {
                if targets
                    .iter()
                    .any(|t| matches!(t, Argument::Value(Type::Unit)))
                {
                    return Err(fail("nonunit excludes unit"));
                }
            }
            Bound::Class { class, targets } => {
                for target in targets {
                    if !matches!(&target, Argument::Value(ty) if class.supports(ty)) {
                        let name = match &target {
                            Argument::Value(ty) => type_name(module, ty),
                            Argument::Heap(id) => module.types[id.0].name.clone(),
                        };
                        return Err(fail(&format!(
                            "{} has no instance for {name}",
                            class.name()
                        )));
                    }
                }
            }
            Bound::Callable {
                target,
                params,
                result,
                effects: contract,
            } => {
                let Argument::Heap(id) = target else {
                    return Err(fail("callable requires a nominal closure type"));
                };
                let TypeKind::Closure { function, .. } = &module.types[id.0].kind else {
                    return Err(fail("callable requires a closure type"));
                };
                if !argument_shared(module, &kinds, &Argument::Heap(id)) {
                    return Err(fail("callable requires a shared, reusable closure"));
                }
                let f = &module.functions[function.0];
                let actual = f
                    .params
                    .iter()
                    .map(|id| f.locals[id.0].ty.clone())
                    .collect::<Vec<_>>();
                if actual != params || f.result != result {
                    return Err(fail(&format!(
                        "callable signature mismatch for `{}`: expected {}, actual {}",
                        module.types[id.0].name,
                        signature(module, &params, &result),
                        signature(module, &actual, &f.result)
                    )));
                }
                if let Some(contract) = contract {
                    effects.push(EffectObligation {
                        site,
                        span,
                        function: *function,
                        contract,
                    });
                }
            }
        }
    }
    Ok(effects)
}

pub(crate) fn check_effects(
    source: &str,
    module: &Module,
    obligations: &[EffectObligation<'_>],
) -> Result<(), Error> {
    if obligations.is_empty() {
        return Ok(());
    }
    let inferred = effects::infer(module);
    for obligation in obligations {
        if let Some(excess) = effects::excess(
            module,
            obligation.function,
            &inferred,
            &obligation.contract.allowed,
        ) {
            let mut error = obligation.site.error(
                source,
                obligation.span,
                format!(
                    "callable effects exceed the allowed bound: {}",
                    excess.names
                ),
            );
            for (span, message) in excess.origins {
                error = error.related(source, span, message);
            }
            return Err(error);
        }
    }
    Ok(())
}
