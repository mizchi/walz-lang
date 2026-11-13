//! Substitution touches types and explicit type arguments only. Term names,
//! strings, annotations and inner type binders keep their own identities.
use super::*;

pub(super) fn apply(
    source: &str,
    node: &mut Sexp,
    bindings: &BTreeMap<String, Sexp>,
) -> Result<(), Error> {
    walk(source, node, bindings, &BTreeSet::new(), &BTreeSet::new())
}
fn walk(
    source: &str,
    node: &mut Sexp,
    bindings: &BTreeMap<String, Sexp>,
    components: &BTreeSet<String>,
    functions: &BTreeSet<String>,
) -> Result<(), Error> {
    let head = node.head().unwrap_or("").to_owned();
    if crate::syntax::annotation(&head) {
        return Ok(());
    }
    let Some(parts) = node.list() else {
        return Ok(());
    };
    let mut bindings = bindings.clone();
    for header in parts.iter().filter(|n| n.head() == Some("@type.params")) {
        for name in header.list().unwrap().iter().skip(1).filter_map(Sexp::atom) {
            bindings.remove(name);
        }
    }
    if bindings.is_empty() {
        return Ok(());
    }
    if head == "@lib" {
        for decl in parts.iter().skip(1) {
            let Some(p) = decl.list() else { continue };
            let names: Vec<_> = match decl.head() {
                Some("type" | "@type.alias") => p.get(1).and_then(Sexp::atom).into_iter().collect(),
                Some("@use" | "@file.use") => p
                    .iter()
                    .skip(2)
                    .filter(|n| n.head() == Some("type"))
                    .filter_map(|n| n.list()?.last()?.atom())
                    .collect(),
                _ => vec![],
            };
            if let Some(name) = names.into_iter().find(|n| bindings.contains_key(*n)) {
                return Err(fail(
                    source,
                    decl,
                    format!(
                        "constructor type parameter `{name}` is redeclared in its library type scope"
                    ),
                ));
            }
        }
    }
    if matches!(head.as_str(), "component" | "@lib") {
        let mut components = components.clone();
        let mut functions = functions.clone();
        if head == "component" {
            components = super::names(&parts[1..]);
            functions.clear();
            for lib in parts.iter().filter(|n| n.head() == Some("@lib")) {
                for f in lib
                    .list()
                    .unwrap()
                    .iter()
                    .skip(1)
                    .filter(|n| n.head() == Some("func"))
                {
                    if let Some(name) = f.list().unwrap().get(1).and_then(Sexp::atom) {
                        functions.insert(name.to_owned());
                    }
                }
            }
        }
        if let Kind::List(p) = &mut node.kind {
            for n in p.iter_mut().skip(1) {
                walk(source, n, &bindings, &components, &functions)?;
            }
        }
        return Ok(());
    }
    // Expand applications in the two Component-only reference positions before
    // visiting their type arguments. Ordinary call/type positions were already
    // handled by the shared contextual angle parser.
    if let Kind::List(p) = &mut node.kind {
        if head == "@expose"
            && let Some(target) = p.get_mut(2)
            && !target.atom().is_some_and(|n| functions.contains(n))
        {
            *target = super::application(source, target)?;
        }
        super::references(node, &mut |target| {
            if !target.atom().is_some_and(|n| components.contains(n)) {
                *target = super::application(source, target)?;
            }
            Ok(())
        })?;
    }
    let mut view = crate::applications::canonical_view(node);
    let mut edits = BTreeMap::new();
    let mut replace = |ty: &mut Sexp| -> Result<(), Error> {
        if let Some(value) = ty.atom().and_then(|n| bindings.get(n)) {
            let mut value = value.clone();
            fn anchor(node: &mut Sexp, span: Span) {
                node.span = span;
                if let Kind::List(p) = &mut node.kind {
                    for node in p {
                        anchor(node, span);
                    }
                }
            }
            anchor(&mut value, ty.span);
            edits.insert(ty.span, value);
        }
        Ok(())
    };
    crate::type_syntax::map_types(&mut view, &mut replace)?;
    let mut heaps = BTreeSet::new();
    crate::type_syntax::map_references(&mut view, &mut |kind, n| {
        if kind == crate::type_syntax::ReferenceKind::Type {
            replace(n)?;
            heaps.insert(n.span);
        }
        Ok(())
    })?;
    fn arguments(
        n: &mut Sexp,
        f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if n.head().is_some_and(crate::syntax::annotation) {
            return Ok(());
        }
        if n.head() == Some("@inst")
            && let Kind::List(p) = &mut n.kind
        {
            for ty in p.iter_mut().skip(2) {
                crate::type_syntax::map_type(&mut crate::applications::canonical_view(ty), f)?;
            }
        }
        if let Kind::List(p) = &mut n.kind {
            for n in p.iter_mut().skip(1) {
                arguments(n, f)?;
            }
        }
        Ok(())
    }
    arguments(node, &mut replace)?;
    for span in heaps {
        if let Some(value) = edits.get_mut(&span)
            && value.head() == Some("ref")
            && let Some(heap) = value.list().and_then(|p| p.last()).cloned()
        {
            *value = heap;
        }
    }
    fn write(n: &mut Sexp, edits: &BTreeMap<Span, Sexp>) {
        if let Some(value) = edits.get(&n.span).filter(|_| n.atom().is_some()) {
            *n = value.clone();
        } else if let Kind::List(p) = &mut n.kind {
            for n in p {
                write(n, edits);
            }
        }
    }
    write(node, &edits);
    Ok(())
}
