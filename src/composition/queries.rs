//! Read-only projections of explicit connections. Type syntax is elaborated
//! before the ordinary checker; querying never adds imports or host authority.
use super::*;
use crate::{component::bound_imports::BoundImports, ir::Span};
use wit_parser::{Type as T, TypeDefKind as K, TypeId};
pub(crate) mod deferred;

fn fail(source: &str, node: &Sexp, message: impl std::fmt::Display) -> Error {
    Error::at(source, node.span, message).with_code("E_COMPONENT_TYPE")
}
fn walk(n: &Sexp, f: &mut impl FnMut(&Sexp) -> Result<(), Error>) -> Result<(), Error> {
    if n.head().is_some_and(crate::syntax::annotation) {
        return Ok(());
    }
    f(n)?;
    if let Some(p) = n.list() {
        for n in p.iter().skip(usize::from(n.head().is_some())) {
            walk(n, f)?;
        }
    }
    Ok(())
}
pub(crate) fn reject_unresolved(source: &str, root: &Sexp) -> Result<(), Error> {
    walk(root, &mut |n| {
        if n.head() == Some("@component.type") {
            return Err(Error::at(
                source,
                n.span,
                "@component.type needs an explicit @bind in a Component implementation",
            )
            .with_code("E_COMPONENT_TYPE_CONTEXT"));
        }
        Ok(())
    })
}
#[derive(Default)]
struct Budget {
    count: usize,
    bytes: usize,
}
pub(super) fn expand(
    source: &str,
    fields: &mut Vec<Sexp>,
    bound: Option<&BoundImports>,
) -> Result<BTreeMap<String, T>, Error> {
    let mut types = BTreeMap::new();
    scope(
        source,
        fields,
        bound,
        &mut Budget::default(),
        "",
        &mut types,
    )?;
    Ok(types)
}
fn scope(
    source: &str,
    fields: &mut Vec<Sexp>,
    bound: Option<&BoundImports>,
    budget: &mut Budget,
    path: &str,
    types: &mut BTreeMap<String, T>,
) -> Result<(), Error> {
    for n in fields
        .iter_mut()
        .filter(|n| matches!(n.head(), Some("@file.namespace" | "@namespace")))
    {
        if let Kind::List(p) = &mut n.kind {
            let mut children = p.split_off(2);
            let child = crate::modules::qualified(path, p[1].atom().unwrap_or(""));
            scope(
                source,
                &mut children,
                bound,
                budget,
                child.trim_start_matches('$'),
                types,
            )?;
            p.extend(children);
        }
    }
    let mut queries = BTreeMap::new();
    for n in fields
        .iter()
        .filter(|n| !matches!(n.head(), Some("@file.namespace" | "@namespace")))
    {
        walk(n, &mut |n| {
            if n.head() == Some("@component.type") {
                queries.insert(n.span, n.clone());
            }
            Ok(())
        })?;
    }
    if queries.is_empty() {
        return Ok(());
    }
    let first = queries.values().next().unwrap();
    let bound = bound.ok_or_else(|| {
        fail(
            source,
            first,
            "Component type queries require an explicit @bind connection",
        )
    })?;
    let mut prefix = format!("$walz-component-query-{}", first.span.start);
    while source.contains(&prefix) {
        prefix.push('x');
    }
    let resources = resources(&bound.resolve, fields);
    let mut schema = crate::wit_bindgen::TypeProjection::new(&bound.resolve, prefix, resources);
    let mut replacements = BTreeMap::new();
    for (span, query) in queries {
        budget.count += 1;
        if budget.count > 1024 {
            return Err(fail(
                source,
                &query,
                "Component type query count limit exceeded",
            ));
        }
        let ty = select(source, &query, bound)?;
        let text = schema
            .ty(ty)
            .map_err(|e| fail(source, &query, &e.diagnostic().message))?;
        budget.bytes = budget.bytes.saturating_add(text.len());
        if budget.bytes > 16 * 1024 * 1024 {
            return Err(fail(
                source,
                &query,
                "Component type query size limit exceeded",
            ));
        }
        let mut ty = crate::sexpr::parse(&text)?;
        anchor(&mut ty, span);
        replacements.insert(span, ty);
    }
    // The shared type visitor identifies positions, including generic arguments
    // and typed lets. A query in an expression/metadata role is not executed.
    let mut typed = BTreeSet::new();
    for n in fields.iter() {
        if matches!(n.head(), Some("@file.namespace" | "@namespace")) {
            continue;
        }
        let mut view = crate::applications::canonical_view(n);
        crate::type_syntax::map_types(&mut view, &mut |n| {
            if n.head() == Some("component.type") {
                typed.insert(n.span);
            }
            Ok(())
        })?;
    }
    fn replace(
        source: &str,
        n: &mut Sexp,
        typed: &BTreeSet<Span>,
        replacements: &BTreeMap<Span, Sexp>,
    ) -> Result<(), Error> {
        if n.head().is_some_and(crate::syntax::annotation) {
            return Ok(());
        }
        if let Some(ty) = replacements
            .get(&n.span)
            .filter(|_| n.head() == Some("@component.type"))
        {
            if !typed.contains(&n.span) {
                return Err(fail(
                    source,
                    n,
                    "@component.type belongs in a value-type position",
                ));
            }
            *n = ty.clone();
        } else {
            let skip = usize::from(n.head().is_some());
            if let Kind::List(p) = &mut n.kind {
                for n in p.iter_mut().skip(skip) {
                    replace(source, n, typed, replacements)?;
                }
            }
        }
        Ok(())
    }
    let origin = replacements.keys().next().copied().unwrap();
    for n in fields.iter_mut() {
        replace(source, n, &typed, &replacements)?;
    }
    let declarations = schema.declarations();
    for (name, ty) in schema.types() {
        types.insert(crate::modules::qualified(path, name), *ty);
    }
    budget.bytes = budget.bytes.saturating_add(declarations.len());
    if budget.bytes > 16 * 1024 * 1024 {
        return Err(
            Error::at(source, origin, "Component type query size limit exceeded")
                .with_code("E_COMPONENT_TYPE"),
        );
    }
    let mut helpers = crate::sexpr::parse(&format!("(module {declarations})"))?;
    anchor(&mut helpers, origin);
    if let Kind::List(p) = helpers.kind {
        fields.splice(0..0, p.into_iter().skip(1));
    }
    Ok(())
}
fn anchor(n: &mut Sexp, span: Span) {
    n.span = span;
    if let Kind::List(p) = &mut n.kind {
        for n in p {
            anchor(n, span);
        }
    }
}
fn resources(r: &wit_parser::Resolve, fields: &[Sexp]) -> BTreeMap<TypeId, String> {
    let mut source = BTreeMap::<String, Vec<String>>::new();
    for n in fields.iter().filter(|n| n.head() == Some("type")) {
        let p = n.list().unwrap();
        if let Some(name) = p.get(1).and_then(Sexp::atom)
            && let Some(kind) = p
                .get(2)
                .filter(|n| n.head() == Some("@resource"))
                .and_then(Sexp::list)
                .and_then(|p| p.get(1))
                .and_then(Sexp::text)
        {
            source.entry(kind.into()).or_default().push(name.into());
        }
    }
    let mut ids = BTreeMap::<String, Vec<TypeId>>::new();
    // A source kind is an assertion, not nominal identity. It may anchor only
    // one actual resource in this graph, with one explicit source declaration.
    // The later World gate also checks destructors and own/borrow signatures.
    for (id, _) in r
        .types
        .iter()
        .filter(|(_, t)| matches!(t.kind, K::Resource))
    {
        if let Ok(kind) = crate::wit_bindgen::resource_kind(r, id) {
            ids.entry(kind).or_default().push(id);
        }
    }
    ids.into_iter()
        .filter_map(|(kind, ids)| {
            let names = source.get(&kind)?;
            (ids.len() == 1 && names.len() == 1).then(|| (ids[0], names[0].clone()))
        })
        .collect()
}
fn select(source: &str, query: &Sexp, bound: &BoundImports) -> Result<Option<T>, Error> {
    let p = query.list().unwrap();
    if p.len() > 67 {
        return Err(fail(
            source,
            query,
            "Component type projection path exceeds 64 steps",
        ));
    }
    let namespace = p.get(1).and_then(Sexp::text).ok_or_else(|| {
        fail(
            source,
            query,
            "expected (@component.type \"namespace\" (param/result/type ...)[ projections...])",
        )
    })?;
    let root = p
        .get(2)
        .ok_or_else(|| fail(source, query, "missing Component type selector"))?;
    let parts = root
        .list()
        .ok_or_else(|| fail(source, root, "expected a param/result/type selector"))?;
    let name = parts.get(1).and_then(Sexp::text).ok_or_else(|| {
        fail(
            source,
            root,
            "selector requires a function or type name string",
        )
    })?;
    let function = || {
        bound
            .functions
            .get(namespace)
            .and_then(|fs| fs.get(name))
            .ok_or_else(|| {
                fail(
                    source,
                    root,
                    format!("no bound Component function {namespace}#{name}"),
                )
            })
    };
    let mut ty = match root.head() {
        Some("param") if parts.len() == 3 => {
            let f = function()?;
            let param = if let Some(name) = parts[2].text() {
                f.params.iter().find(|p| p.name == name)
            } else {
                parts[2]
                    .atom()
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| f.params.get(i))
            };
            Some(
                param
                    .ok_or_else(|| fail(source, &parts[2], "no such Component function parameter"))?
                    .ty,
            )
        }
        Some("result") if parts.len() == 2 => function()?.result,
        Some("type") if parts.len() == 2 => {
            let id = bound.interfaces.get(namespace).ok_or_else(|| {
                fail(source, root, "named type queries require an instance @bind")
            })?;
            Some(T::Id(
                *bound.resolve.interfaces[*id]
                    .types
                    .get(name)
                    .ok_or_else(|| {
                        fail(
                            source,
                            root,
                            format!("no bound Component type {namespace}#{name}"),
                        )
                    })?,
            ))
        }
        _ => {
            return Err(fail(
                source,
                root,
                "expected (param \"function\" index-or-name), (result \"function\"), or (type \"name\")",
            ));
        }
    };
    for selector in p.iter().skip(3) {
        ty = project(source, &bound.resolve, ty, selector)?;
    }
    Ok(ty)
}
fn project(
    source: &str,
    r: &wit_parser::Resolve,
    ty: Option<T>,
    n: &Sexp,
) -> Result<Option<T>, Error> {
    let bad = || {
        fail(
            source,
            n,
            "Component type projection does not match the selected type",
        )
    };
    let mut ty = ty.ok_or_else(bad)?;
    for _ in 0..64 {
        if let T::Id(id) = ty
            && let K::Type(next) = r.types[id].kind
        {
            ty = next;
        } else {
            break;
        }
    }
    let T::Id(id) = ty else { return Err(bad()) };
    let kind = &r.types[id].kind;
    match (n.atom(), kind) {
        (Some("option.element"), K::Option(t)) | (Some("list.element"), K::List(t)) => {
            return Ok(Some(*t));
        }
        (Some("result.ok"), K::Result(t)) => return Ok(t.ok),
        (Some("result.err"), K::Result(t)) => return Ok(t.err),
        (Some("future.element"), K::Future(t)) | (Some("stream.element"), K::Stream(t)) => {
            return Ok(*t);
        }
        (Some("tuple.first"), K::Tuple(t)) if t.types.len() == 2 => return Ok(Some(t.types[0])),
        (Some("tuple.second"), K::Tuple(t)) if t.types.len() == 2 => return Ok(Some(t.types[1])),
        _ => {}
    }
    if let Some([_, member]) = n.list() {
        match (n.head(), kind) {
            (Some("field"), K::Record(t)) => {
                return t
                    .fields
                    .iter()
                    .find(|f| Some(f.name.as_str()) == member.text())
                    .map(|f| Some(f.ty))
                    .ok_or_else(bad);
            }
            (Some("case"), K::Variant(t)) => {
                return t
                    .cases
                    .iter()
                    .find(|c| Some(c.name.as_str()) == member.text())
                    .map(|c| c.ty)
                    .ok_or_else(bad);
            }
            (Some("item"), K::Tuple(t)) => {
                return member
                    .atom()
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| t.types.get(i))
                    .copied()
                    .map(Some)
                    .ok_or_else(bad);
            }
            _ => {}
        }
    }
    Err(bad())
}
