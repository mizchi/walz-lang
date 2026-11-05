//! Explicit, bounded source monomorphization. Only concrete declarations reach
//! the existing type/effect/ownership checkers and executable IR.
use crate::{
    Error,
    ir::Span,
    sexpr::{Kind, Sexp},
    type_syntax::{self, ReferenceKind},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_INSTANCES: usize = 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 1_000_000;

fn identifier(source: &str, node: &Sexp) -> Result<String, Error> {
    let name = node.atom().unwrap_or("");
    if !name.starts_with('$')
        || name.len() < 2
        || name.starts_with("$__")
        || !name[1..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return Err(Error::at(source, node.span, "expected ASCII $identifier"));
    }
    Ok(name.into())
}

/// The binder has one position, immediately after the declaration's name.
pub(crate) fn parameters(source: &str, node: &Sexp) -> Result<Vec<String>, Error> {
    let Some(items) = node.list() else {
        return Ok(vec![]);
    };
    let groups = items
        .iter()
        .enumerate()
        .filter(|(_, n)| n.head() == Some("type.params"))
        .collect::<Vec<_>>();
    if groups.is_empty() {
        return Ok(vec![]);
    }
    if groups.len() != 1
        || groups[0].0 != 2
        || !matches!(
            node.head(),
            Some("func" | "type" | "type.alias" | "closure")
        )
    {
        return Err(Error::at(
            source,
            node.span,
            "type.params belongs immediately after a func/type/type.alias/closure name",
        ));
    }
    let group = groups[0].1;
    let mut names = BTreeSet::new();
    let mut out = vec![];
    for parameter in &group.list().unwrap()[1..] {
        let name = identifier(source, parameter)?;
        if !names.insert(name.clone()) {
            return Err(Error::at(
                source,
                parameter.span,
                "duplicate type parameter",
            ));
        }
        out.push(name);
    }
    if out.is_empty() {
        return Err(Error::at(
            source,
            group.span,
            "type.params requires at least one parameter",
        ));
    }
    Ok(out)
}

#[derive(Clone)]
struct Template {
    syntax: Sexp,
    parameters: Vec<String>,
}

pub(crate) struct Expanded {
    pub syntax: Sexp,
    /// Nominal specializations inherit their template's representation privacy.
    pub origins: BTreeMap<String, String>,
    instances: Vec<(Span, Span, String)>,
}
impl Expanded {
    pub fn annotate(&self, source: &str, mut error: Error) -> Error {
        if let Some(primary) = &error.diagnostic().primary {
            let span = primary.span;
            for (_, call, name) in self
                .instances
                .iter()
                .filter(|(definition, _, _)| {
                    definition.start <= span.start && span.end <= definition.end
                })
                .take(8)
            {
                error = error.related(
                    source,
                    *call,
                    format!("instantiation of {name} requested here"),
                );
            }
        }
        error
    }
}

struct Expander<'a> {
    source: &'a str,
    templates: Vec<Template>,
    definitions: BTreeMap<(ReferenceKind, String), usize>,
    names: BTreeSet<String>,
    nominal: BTreeSet<String>,
    aliases: BTreeMap<String, Sexp>,
    cache: BTreeMap<(usize, Vec<String>), String>,
    active: Vec<usize>,
    pending: VecDeque<(Sexp, Vec<usize>, Span)>,
    output: Vec<Sexp>,
    origins: BTreeMap<String, String>,
    instances: Vec<(Span, Span, String)>,
    nodes: usize,
}

fn key(node: &Sexp) -> String {
    match &node.kind {
        Kind::Atom(s) => s.clone(),
        Kind::List(items) => format!("({})", items.iter().map(key).collect::<Vec<_>>().join(" ")),
        Kind::Text(_) => unreachable!("type arguments cannot contain strings"),
    }
}

impl Expander<'_> {
    fn error(&self, node: &Sexp, message: impl std::fmt::Display) -> Error {
        Error::at(self.source, node.span, message).with_code("E_GENERIC")
    }
    fn charge(&mut self, node: &Sexp) -> Result<(), Error> {
        let mut pending = vec![node];
        while let Some(n) = pending.pop() {
            self.nodes += 1;
            if self.nodes > MAX_NODES {
                return Err(self.error(node, "generic expansion node limit exceeded"));
            }
            if let Some(items) = n.list() {
                pending.extend(items);
            }
        }
        Ok(())
    }
    fn normalize(&mut self, node: &Sexp, depth: usize) -> Result<Sexp, Error> {
        self.nodes += 1;
        if depth > MAX_DEPTH || self.nodes > MAX_NODES {
            return Err(self.error(node, "generic type argument expansion limit exceeded"));
        }
        if let Some(name) = node.atom() {
            if self
                .definitions
                .contains_key(&(ReferenceKind::Type, name.to_owned()))
            {
                return self.apply(ReferenceKind::Type, node);
            }
            if let Some(target) = self.aliases.get(name).cloned() {
                return self.normalize(&target, depth + 1);
            }
            if matches!(
                name,
                "i32" | "i64" | "f32" | "f64" | "unit" | "bytes" | "text"
            ) || self.nominal.contains(name)
            {
                return Ok(node.clone());
            }
            return Err(self.error(node, format!("unknown or unapplied type argument `{name}`")));
        }
        if node.head().is_some_and(|h| h.starts_with('$')) {
            let concrete = self.apply(ReferenceKind::Type, node)?;
            return self.normalize(&concrete, depth + 1);
        }
        let mut out = node.clone();
        let head = node.head().unwrap_or("");
        let expected = match head {
            "list" | "option" | "own" | "future" | "stream" | "borrow" | "resource" => 2,
            "result" | "pair" => 3,
            "ref"
                if node
                    .list()
                    .is_some_and(|p| p.len() == 3 && p[1].atom() == Some("null")) =>
            {
                3
            }
            "ref" => 2,
            _ => return Err(self.error(node, "expected a concrete type argument")),
        };
        let Kind::List(items) = &mut out.kind else {
            unreachable!()
        };
        if items.len() != expected {
            return Err(self.error(node, "invalid type argument arity"));
        }
        let start = if head == "ref" { expected - 1 } else { 1 };
        for ty in &mut items[start..] {
            *ty = self.normalize(ty, depth + 1)?;
        }
        if matches!(head, "ref" | "borrow" | "resource")
            && !items
                .last()
                .and_then(Sexp::atom)
                .is_some_and(|n| self.nominal.contains(n))
        {
            return Err(self.error(node, "reference type arguments require a nominal heap type"));
        }
        Ok(out)
    }
    fn apply(&mut self, function: ReferenceKind, node: &Sexp) -> Result<Sexp, Error> {
        let name = node.atom().or_else(|| node.head()).unwrap_or("");
        if node.atom().is_none() && !name.starts_with('$') {
            return Ok(node.clone());
        }
        let Some(&index) = self.definitions.get(&(function, name.into())) else {
            if node.atom().is_some() {
                return Ok(node.clone());
            }
            return Err(self.error(
                node,
                format!(
                    "`{name}` is not a generic {}",
                    if function == ReferenceKind::Function {
                        "function"
                    } else {
                        "type"
                    }
                ),
            ));
        };
        let template = self.templates[index].clone();
        let args = node.list().map(|p| &p[1..]).unwrap_or(&[]);
        if node.atom().is_some() || args.len() != template.parameters.len() {
            return Err(self
                .error(
                    node,
                    format!(
                        "generic `{name}` requires {} explicit type arguments, got {}",
                        template.parameters.len(),
                        args.len()
                    ),
                )
                .related(self.source, template.syntax.span, "generic declaration"));
        }
        let args = args
            .iter()
            .map(|arg| self.normalize(arg, 0))
            .collect::<Result<Vec<_>, _>>()?;
        let keys = args.iter().map(key).collect::<Vec<_>>();
        let cache_key = (index, keys.clone());
        let concrete = if let Some(name) = self.cache.get(&cache_key) {
            name.clone()
        } else {
            if self.active.contains(&index) {
                return Err(self.error(node, "polymorphic recursion changes type arguments; specialization must remain finite")
                    .related(self.source, template.syntax.span, "recursive generic declaration"));
            }
            if self.cache.len() >= MAX_INSTANCES || self.active.len() >= MAX_DEPTH {
                return Err(self.error(node, "generic specialization limit exceeded"));
            }
            let mut suffix = self.cache.len();
            let concrete = loop {
                let candidate = format!("{name}.mono{suffix}");
                if self.names.insert(candidate.clone()) {
                    break candidate;
                }
                suffix += 1;
            };
            self.cache.insert(cache_key, concrete.clone());
            self.origins.insert(concrete.clone(), name.into());
            if self.nominal.contains(name) {
                self.nominal.insert(concrete.clone());
            }
            let mut syntax = template.syntax.clone();
            self.charge(&syntax)?;
            let Kind::List(parts) = &mut syntax.kind else {
                unreachable!()
            };
            parts[1].kind = Kind::Atom(concrete.clone());
            parts.remove(2);
            // Rename bound type variables before inserting caller types. Those
            // types may contain a nominal name identical to a bound variable.
            // The placeholders cannot be written as source identifiers.
            let placeholders = template
                .parameters
                .iter()
                .enumerate()
                .map(|(i, _)| format!("@generic.{index}.{i}"))
                .collect::<Vec<_>>();
            let renames = template
                .parameters
                .iter()
                .zip(&placeholders)
                .collect::<BTreeMap<_, _>>();
            let mut rename = |ty: &mut Sexp| {
                if let Some(name) = ty.atom().and_then(|n| renames.get(&n.to_owned())) {
                    ty.kind = Kind::Atom((*name).clone());
                }
                Ok(())
            };
            type_syntax::map_types(&mut syntax, &mut rename)?;
            type_syntax::map_references(&mut syntax, &mut |_, n| rename(n))?;
            let bindings = placeholders.iter().zip(&args).collect::<BTreeMap<_, _>>();
            let mut substitute = |ty: &mut Sexp| {
                if let Some(value) = ty.atom().and_then(|n| bindings.get(&n.to_owned())) {
                    self.charge(value)?;
                    let span = ty.span;
                    *ty = (*value).clone();
                    ty.span = span;
                }
                Ok(())
            };
            type_syntax::map_types(&mut syntax, &mut substitute)?;
            type_syntax::map_references(&mut syntax, &mut |_, n| substitute(n))?;
            if syntax.head() == Some("type.alias") {
                self.aliases
                    .insert(concrete.clone(), syntax.list().unwrap()[2].clone());
            }
            let mut ancestry = self.active.clone();
            ancestry.push(index);
            self.pending.push_back((syntax, ancestry, node.span));
            concrete
        };
        self.instances.push((
            template.syntax.span,
            node.span,
            format!("{name}<{}>", keys.join(", ")),
        ));
        Ok(Sexp {
            kind: Kind::Atom(concrete),
            span: node.span,
        })
    }
    fn walk(&mut self, node: &mut Sexp) -> Result<(), Error> {
        if node.head() == Some("export")
            && let Kind::List(parts) = &mut node.kind
            && let Some(descriptor) = parts.get_mut(2)
            && let Kind::List(parts) = &mut descriptor.kind
            && let Some(target) = parts.get_mut(1)
        {
            *target = self.apply(ReferenceKind::Function, target)?;
        }
        type_syntax::map_types(node, &mut |ty| {
            *ty = self.apply(ReferenceKind::Type, ty)?;
            Ok(())
        })?;
        type_syntax::map_references(node, &mut |function, operand| {
            *operand = self.apply(function, operand)?;
            Ok(())
        })
    }
}

pub(crate) fn expand(source: &str, root: &Sexp) -> Result<Expanded, Error> {
    let Some(items) = root.list().filter(|_| root.head() == Some("module")) else {
        return Err(Error::at(source, root.span, "expected (module ...)"));
    };
    let mut e = Expander {
        source,
        templates: vec![],
        definitions: BTreeMap::new(),
        names: BTreeSet::new(),
        nominal: BTreeSet::new(),
        aliases: BTreeMap::new(),
        cache: BTreeMap::new(),
        active: vec![],
        pending: VecDeque::new(),
        output: vec![],
        origins: BTreeMap::new(),
        instances: vec![],
        nodes: 0,
    };
    e.charge(root)?;
    let mut ordinary = vec![items[0].clone()];
    for node in &items[1..] {
        let parameters = parameters(source, node)?;
        let declaration = if node.head() == Some("import") {
            node.list().and_then(|p| p.get(3)).unwrap_or(node)
        } else {
            node
        };
        if declaration.head() == Some("func")
            && !self::parameters(source, declaration)?.is_empty()
            && declaration.span != node.span
        {
            return Err(e.error(node, "host imports require concrete signatures"));
        }
        if let Some(name) = node.list().and_then(|p| p.get(1)).and_then(Sexp::atom) {
            e.names.insert(name.into());
            if matches!(node.head(), Some("type" | "closure")) {
                e.nominal.insert(name.into());
            }
            if node.head() == Some("type.alias")
                && parameters.is_empty()
                && node.list().unwrap().len() == 3
            {
                e.aliases
                    .insert(name.into(), node.list().unwrap()[2].clone());
            }
        }
        if parameters.is_empty() {
            ordinary.push(node.clone());
            continue;
        }
        let parts = node.list().unwrap();
        let name = identifier(source, &parts[1])?;
        if parts.iter().any(|n| n.head() == Some("export")) {
            return Err(e.error(node, "generic declarations cannot have Wasm exports; export a concrete application or wrapper"));
        }
        if parts.len() < 4 || matches!(node.head(), Some("type" | "type.alias")) && parts.len() != 4
        {
            return Err(e.error(node, "invalid generic declaration"));
        }
        for function in match node.head() {
            Some("closure") => &[ReferenceKind::Type, ReferenceKind::Function][..],
            Some("func") => &[ReferenceKind::Function][..],
            _ => &[ReferenceKind::Type][..],
        } {
            if e.definitions
                .insert((*function, name.clone()), e.templates.len())
                .is_some()
            {
                return Err(e.error(node, "duplicate generic declaration"));
            }
        }
        e.templates.push(Template {
            syntax: node.clone(),
            parameters,
        });
    }
    for node in ordinary.iter_mut().skip(1) {
        e.walk(node)?;
    }
    // A work queue keeps nested specialization off the Rust call stack.
    while let Some((mut syntax, ancestry, requested)) = e.pending.pop_front() {
        e.active = ancestry;
        e.walk(&mut syntax)
            .map_err(|error| error.related(source, requested, "specialization requested here"))?;
        e.output.push(syntax);
    }
    ordinary.append(&mut e.output);
    // Declarative exports can select a concrete function specialization.
    let exports = ordinary
        .iter()
        .filter(|n| n.head() == Some("export"))
        .cloned()
        .collect::<Vec<_>>();
    ordinary.retain(|n| n.head() != Some("export"));
    for export in exports {
        let parts = export.list().unwrap();
        let target = parts
            .get(2)
            .and_then(Sexp::list)
            .and_then(|d| d.get(1))
            .and_then(Sexp::atom);
        let definition = ordinary
            .iter_mut()
            .find(|n| {
                n.head() == Some("func")
                    && n.list().and_then(|p| p.get(1)).and_then(Sexp::atom) == target
            })
            .ok_or_else(|| e.error(&export, "exports require a concrete defined function"))?;
        let Kind::List(parts) = &mut definition.kind else {
            unreachable!()
        };
        parts.insert(
            2,
            Sexp {
                kind: Kind::List(export.list().unwrap()[..2].to_vec()),
                span: export.span,
            },
        );
    }
    Ok(Expanded {
        syntax: Sexp {
            kind: Kind::List(ordinary),
            span: root.span,
        },
        origins: e.origins,
        instances: e.instances,
    })
}
