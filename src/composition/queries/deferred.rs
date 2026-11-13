//! Argument discovery uses private witnesses, never implementation IR. Only
//! queries reachable from the requested arguments are elaborated in a stage.
use super::*;
use crate::ir::{Module, Type, TypeKind, ValueType};
pub(crate) struct Environment {
    original: Sexp,
    original_source: String,
    marker: String,
    pub source: String,
    pub queries: BTreeMap<String, Sexp>,
}
pub(crate) fn environment(source: String, original: &str) -> Result<Environment, Error> {
    let mut marker = "$walz-deferred-component-type".to_owned();
    while original.contains(&marker) {
        marker.push('x');
    }
    let original_source = format!("(module {source})");
    let original = crate::sexpr::parse(&original_source)?;
    let mut syntax = original.clone();
    let mut queries = BTreeMap::new();
    prepare(
        &mut syntax,
        &marker,
        "",
        &mut 0,
        &BTreeSet::new(),
        &mut queries,
    );
    Ok(Environment {
        original,
        original_source,
        marker,
        source: body(&syntax),
        queries,
    })
}
fn body(n: &Sexp) -> String {
    n.list().unwrap()[1..]
        .iter()
        .map(render)
        .collect::<Vec<_>>()
        .join("\n")
}
fn prepare(
    n: &mut Sexp,
    marker: &str,
    path: &str,
    count: &mut usize,
    keep: &BTreeSet<Span>,
    queries: &mut BTreeMap<String, Sexp>,
) {
    let Some(p) = n.list() else { return };
    let start = if n.head() == Some("module") { 1 } else { 2 };
    let mut children = p.iter().skip(start).cloned().collect::<Vec<_>>();
    let mut declarations = vec![];
    fn replace(
        n: &mut Sexp,
        marker: &str,
        path: &str,
        count: &mut usize,
        keep: &BTreeSet<Span>,
        queries: &mut BTreeMap<String, Sexp>,
        declarations: &mut Vec<Sexp>,
    ) {
        if n.head().is_some_and(crate::syntax::annotation) {
            return;
        }
        if n.head() == Some("@component.type") {
            let name = format!("{marker}{}", *count);
            *count += 1;
            queries.insert(crate::modules::qualified(path, &name), n.clone());
            if !keep.contains(&n.span) {
                n.kind = Kind::Atom(name.clone());
                // Discovery witnesses are portable scalars so annotations in
                // enclosing records remain checkable; the real graph is fully
                // rechecked after selected queries replace the witnesses.
                let defs = crate::sexpr::parse(&format!(
                    "(module (type {name} (@newtype i32)) (@abi.type {name} (u32)))"
                ))
                .unwrap();
                declarations.extend_from_slice(&defs.list().unwrap()[1..]);
            }
        } else if matches!(n.head(), Some("@file.namespace" | "@namespace")) {
            let child = crate::modules::qualified(path, n.list().unwrap()[1].atom().unwrap_or(""));
            prepare(
                n,
                marker,
                child.trim_start_matches('$'),
                count,
                keep,
                queries,
            );
        } else if let Kind::List(p) = &mut n.kind {
            for n in p {
                replace(n, marker, path, count, keep, queries, declarations);
            }
        }
    }
    for n in &mut children {
        replace(n, marker, path, count, keep, queries, &mut declarations);
    }
    if let Kind::List(p) = &mut n.kind {
        p.truncate(start);
        p.extend(declarations);
        p.extend(children);
    }
}
impl Environment {
    pub fn needed(&self, module: &Module, types: &[Type]) -> BTreeSet<String> {
        let mut pending = types.iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut needed = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            let id = match ty {
                Type::Newtype(id)
                | Type::Value(id)
                | Type::Ref { ty: id, .. }
                | Type::Resource(id)
                | Type::Borrow(id) => *id,
                _ => continue,
            };
            if !seen.insert(id) {
                continue;
            }
            let decl = &module.types[id.0];
            if self.queries.contains_key(&decl.name) {
                needed.insert(decl.name.clone());
            }
            match &decl.kind {
                TypeKind::Struct(f) | TypeKind::Closure { captures: f, .. } => {
                    pending.extend(f.iter().map(|f| &f.ty))
                }
                TypeKind::Array(f) => pending.push(&f.ty),
                TypeKind::Variant(c) => pending.extend(c.iter().flat_map(|c| &c.payloads)),
                TypeKind::Value(v) => match v {
                    ValueType::List(t)
                    | ValueType::Option(t)
                    | ValueType::Own(t)
                    | ValueType::Future(t)
                    | ValueType::Stream(t)
                    | ValueType::Task(t) => pending.push(t),
                    ValueType::Pair { first, second } => pending.extend([first, second]),
                    ValueType::Result { ok, err } => pending.extend([ok, err]),
                    _ => {}
                },
                _ => {}
            }
        }
        needed
    }
    pub fn resolve(
        &self,
        needed: &BTreeSet<String>,
        bound: &BoundImports,
    ) -> Result<(String, BTreeMap<String, T>), Error> {
        let keep = needed.iter().map(|n| self.queries[n].span).collect();
        let mut syntax = self.original.clone();
        prepare(
            &mut syntax,
            &self.marker,
            "",
            &mut 0,
            &keep,
            &mut BTreeMap::new(),
        );
        let Kind::List(p) = &mut syntax.kind else {
            unreachable!()
        };
        let mut fields = p.split_off(1);
        let types = super::expand(&self.original_source, &mut fields, Some(bound))?;
        Ok((
            fields.iter().map(render).collect::<Vec<_>>().join("\n"),
            types,
        ))
    }
}
