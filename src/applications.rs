//! Type-position lowering of angle sugar, before ordinary syntax validation.
use crate::{
    Error,
    ir::Span,
    sexpr::{Kind, Sexp},
    type_syntax::ReferenceKind,
};
use std::collections::{BTreeMap, BTreeSet};

fn canonical_view(node: &Sexp) -> Sexp {
    let mut out = node.clone();
    fn walk(n: &mut Sexp) {
        let raw = n.head().unwrap_or("").to_owned();
        if crate::syntax::annotation(&raw) {
            return;
        }
        if let Kind::List(items) = &mut n.kind {
            if raw == "@inst" {
                items.remove(0);
            } else if let Some(name) = raw.strip_prefix('@') {
                items[0].kind = Kind::Atom(match name {
                    "struct.new" => "struct.named".into(),
                    "file.use" => "use".into(),
                    "legacy.namespace" => "namespace".into(),
                    _ => crate::naming::core(name).into_owned(),
                });
            }
            for child in items {
                walk(child);
            }
        }
    }
    walk(&mut out);
    out
}

#[derive(Default)]
struct Names {
    types: BTreeSet<String>,
    functions: BTreeSet<String>,
}
fn insert_name(names: &mut BTreeSet<String>, name: &str) {
    names.insert(name.into());
    let mut end = 0;
    if let Ok(Some(token)) = wast::lexer::Lexer::new(name).parse(&mut end)
        && token.kind == wast::lexer::TokenKind::Id
        && end == name.len()
        && let Ok(decoded) = token.id(name)
    {
        names.insert(format!("${decoded}"));
    }
}
fn collect(n: &Sexp, names: &mut Names) {
    let Some(p) = n.list() else { return };
    let kind = n.head().unwrap_or("");
    if matches!(kind, "type" | "type.alias" | "closure")
        && let Some(name) = p.get(1).and_then(Sexp::atom)
    {
        insert_name(&mut names.types, name);
    }
    if matches!(kind, "func" | "closure")
        && let Some(name) = p.get(1).and_then(Sexp::atom)
    {
        insert_name(&mut names.functions, name);
    }
    if matches!(kind, "use") {
        for binding in p.get(2..).unwrap_or_default() {
            if let Some(parts) = binding.list()
                && let Some(name) = parts.last().and_then(Sexp::atom)
            {
                match binding.head() {
                    Some("type") => {
                        insert_name(&mut names.types, name);
                    }
                    Some("func") => {
                        insert_name(&mut names.functions, name);
                    }
                    _ => {}
                }
            }
        }
    }
    if matches!(kind, "import" | "rec") {
        for child in &p[1..] {
            collect(child, names);
        }
    }
}

pub(crate) fn expand(source: &str, root: &mut Sexp, recover: bool) -> Result<(), Error> {
    let view = canonical_view(root);
    let mut edits = BTreeMap::new();
    fn scope(
        source: &str,
        n: &Sexp,
        edits: &mut BTreeMap<Span, Sexp>,
        recover: bool,
    ) -> Result<(), Error> {
        let parts = n.list().unwrap_or(&[]);
        let declarations = if matches!(n.head(), Some("module" | "namespace" | "file.namespace")) {
            parts
                .get(if n.head() == Some("module") { 1 } else { 2 }..)
                .unwrap_or_default()
        } else {
            std::slice::from_ref(n)
        };
        let mut names = Names::default();
        for decl in declarations {
            collect(decl, &mut names);
        }
        for decl in declarations {
            if matches!(decl.head(), Some("namespace" | "file.namespace")) {
                scope(source, decl, edits, recover)?;
                continue;
            }
            let mut decl = decl.clone();
            let mut visit = |kind: ReferenceKind, node: &mut Sexp| -> Result<(), Error> {
                let Some(text) = node
                    .atom()
                    .filter(|s| crate::sexpr::applications::prefix(s).is_some())
                else {
                    return Ok(());
                };
                if match kind {
                    ReferenceKind::Type => &names.types,
                    ReferenceKind::Function => &names.functions,
                }
                .contains(text)
                {
                    return Ok(());
                }
                let result = crate::sexpr::applications::parse(source, node, &names.types);
                match result {
                    Ok(out) => {
                        edits.insert(node.span, out);
                    }
                    Err(error) if !recover => return Err(error),
                    _ => {}
                }
                Ok(())
            };
            crate::type_syntax::map_types(&mut decl, &mut |node| visit(ReferenceKind::Type, node))?;
            crate::type_syntax::map_references(&mut decl, &mut visit)?;
            if decl.head() == Some("export")
                && let Kind::List(p) = &mut decl.kind
                && let Some(target) = p.get_mut(2).filter(|n| n.head() == Some("func"))
                && let Kind::List(p) = &mut target.kind
                && let Some(target) = p.get_mut(1)
            {
                visit(ReferenceKind::Function, target)?;
            }
        }
        Ok(())
    }
    scope(source, &view, &mut edits, recover)?;
    fn apply(n: &mut Sexp, edits: &BTreeMap<Span, Sexp>) {
        if n.atom().is_some()
            && let Some(out) = edits.get(&n.span)
        {
            *n = out.clone();
            return;
        }
        if let Kind::List(items) = &mut n.kind {
            for child in items {
                apply(child, edits);
            }
        }
    }
    apply(root, &edits);
    Ok(())
}
