//! Embedded `wlt:std`: ordinary static aliases and library modules. Resolving
//! this package never performs filesystem or host I/O and adds no runtime.
use crate::{
    Error,
    sexpr::{Kind, Sexp},
};
use std::collections::{BTreeMap, BTreeSet};

use crate::standard_sources::{LIBRARIES, TYPES};

fn imports<'a>(node: &'a Sexp, out: &mut Vec<&'a Sexp>) {
    if node.head() == Some("%import") && node.list().unwrap().get(2).and_then(Sexp::text).is_none()
    {
        out.push(node);
    } else if matches!(
        node.head(),
        Some("module" | "%namespace" | "%file.namespace")
    ) {
        for child in node.list().unwrap().iter().skip(1) {
            imports(child, out);
        }
    }
}
fn exports(node: &Sexp, namespace: &str, out: &mut BTreeSet<(String, String)>) {
    match node.head() {
        Some("%export" | "%public") => {
            for item in &node.list().unwrap()[1..] {
                let parts = item.list().unwrap();
                let kind = if item.head() == Some("opaque") {
                    "type"
                } else {
                    parts[0].atom().unwrap()
                };
                let name = parts[1].atom().unwrap();
                out.insert((
                    kind.into(),
                    if namespace.is_empty() {
                        name.into()
                    } else {
                        format!("${namespace}.{}", &name[1..])
                    },
                ));
            }
        }
        Some("module") => {
            for child in &node.list().unwrap()[1..] {
                exports(child, namespace, out);
            }
        }
        Some("%namespace") => {
            let parts = node.list().unwrap();
            let name = parts[1].atom().unwrap().trim_start_matches('$');
            let name = if namespace.is_empty() {
                name.into()
            } else {
                format!("{namespace}.{name}")
            };
            for child in &parts[2..] {
                exports(child, &name, out);
            }
        }
        _ => {}
    }
}
fn body(source: &str) -> Result<&str, Error> {
    let root = crate::sexpr::parse(source)?;
    let parts = root
        .list()
        .filter(|_| root.head() == Some("module"))
        .ok_or_else(|| Error::message("invalid embedded standard module".into()))?;
    Ok(&source[parts[0].span.end..root.span.end - 1])
}

struct Libraries<'a> {
    prefix: &'a str,
    names: BTreeMap<String, String>,
    active: BTreeSet<String>,
    units: Vec<String>,
    core_types: BTreeSet<String>,
}
impl Libraries<'_> {
    fn load(&mut self, file: &str) -> Result<String, Error> {
        if self.active.contains(file) {
            return Err(Error::message("embedded standard module cycle".into()));
        }
        if let Some(name) = self.names.get(file) {
            return Ok(name.clone());
        }
        let text = LIBRARIES
            .iter()
            .find(|(name, _)| *name == file)
            .map(|(_, text)| *text)
            .ok_or_else(|| Error::message(format!("unknown embedded standard module {file}")))?;
        let name = format!("{}-{}", self.prefix, file.trim_end_matches(".wlt"));
        self.active.insert(file.into());
        let syntax = crate::sexpr::parse(text)?;
        let mut nodes = vec![];
        imports(&syntax, &mut nodes);
        let mut edits = vec![];
        for node in nodes {
            let path = &node.list().unwrap()[1];
            let target = if path.text() == Some("wlt:std") {
                for binding in &node.list().unwrap()[2..] {
                    let fields = binding.list().unwrap();
                    if binding.head() != Some("type") {
                        return Err(Error::message("embedded std imports must use direct library dependencies for functions".into()));
                    }
                    self.core_types.insert(fields[1].atom().unwrap().to_owned());
                }
                self.prefix.to_owned()
            } else {
                self.load(
                    path.text()
                        .ok_or_else(|| Error::message("invalid embedded import".into()))?,
                )?
            };
            edits.push((path.span, target));
        }
        let mut text = text.to_owned();
        edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
        for (span, name) in edits {
            text.replace_range(span.start..span.end, &name);
        }
        self.units
            .push(format!("\n(%file.namespace {name} {})\n", body(&text)?));
        self.active.remove(file);
        self.names.insert(file.into(), name.clone());
        Ok(name)
    }
}

pub(crate) struct Prepared {
    text: Option<String>,
    namespace: String,
    insert: usize,
    line_offset: usize,
}
impl Prepared {
    pub fn source<'a>(&'a self, original: &'a str) -> &'a str {
        self.text.as_deref().unwrap_or(original)
    }
    pub fn rewrite(&self, node: &mut Sexp) {
        if !matches!(node.head(), Some("module" | "namespace" | "file.namespace")) {
            return;
        }
        let Kind::List(parts) = &mut node.kind else {
            return;
        };
        // Textual includes can repeat the same explicit standard binding. Its
        // identity is shared; conflicting bindings still reach normal checking.
        let mut seen = BTreeSet::new();
        parts.retain_mut(|child| {
            if child.head() == Some("import")
                && child.list().unwrap().get(1).and_then(Sexp::text) == Some("wlt:std")
                && child.list().unwrap().get(2).and_then(Sexp::text).is_none()
            {
                let Kind::List(fields) = &mut child.kind else {
                    unreachable!()
                };
                fields[1].kind = Kind::Atom(self.namespace.clone());
                let bindings = fields.split_off(2);
                fields.extend(bindings.into_iter().filter(|binding| {
                    let fields = binding.list().unwrap();
                    seen.insert((
                        binding.head().unwrap().to_owned(),
                        fields[1].atom().unwrap().to_owned(),
                        fields.last().unwrap().atom().unwrap().to_owned(),
                    ))
                }));
                fields.len() > 2
            } else {
                self.rewrite(child);
                true
            }
        });
    }
    pub fn annotate(&self, mut error: Error) -> Error {
        if self.text.is_some() {
            for label in error
                .diagnostic
                .primary
                .iter_mut()
                .chain(&mut error.diagnostic.related)
            {
                if label.location.path.is_none() && label.span.start >= self.insert {
                    label.location.path = Some("wlt:std".into());
                    label.location.line = label.location.line.saturating_sub(self.line_offset);
                }
            }
        }
        error
    }
}

pub(crate) fn prepare(source: &str) -> Result<Prepared, Error> {
    let root = crate::sexpr::parse(source)?;
    let mut nodes = vec![];
    imports(&root, &mut nodes);
    let nodes = nodes
        .into_iter()
        .filter(|n| {
            n.list()
                .unwrap()
                .get(1)
                .and_then(Sexp::text)
                .is_some_and(|s| s.starts_with("wlt:"))
        })
        .collect::<Vec<_>>();
    let mut result = Prepared {
        text: None,
        namespace: String::new(),
        insert: root.span.end.saturating_sub(1),
        line_offset: 0,
    };
    if nodes.is_empty() {
        return Ok(result);
    }
    let mut names = BTreeSet::new();
    let mut pending = vec![&root];
    while let Some(node) = pending.pop() {
        if let Some(atom) = node.atom() {
            names.insert(atom);
        }
        if let Some(parts) = node.list() {
            pending.extend(parts);
        }
    }
    let mut next = 0;
    let prefix = loop {
        let prefix = format!("$wlt-std-{next}");
        next += 1;
        if !names.iter().any(|n| n.starts_with(prefix.as_str())) {
            break prefix;
        }
    };
    let types = crate::sexpr::parse(TYPES)?;
    let mut available_types = BTreeSet::new();
    exports(&types, "", &mut available_types);
    let mut catalogue = BTreeMap::new();
    for (file, text) in LIBRARIES {
        let mut members = BTreeSet::new();
        exports(&crate::sexpr::parse(text)?, "", &mut members);
        for member in members {
            catalogue.insert(member, *file);
        }
    }
    let mut libraries = Libraries {
        prefix: &prefix,
        names: BTreeMap::new(),
        active: BTreeSet::new(),
        units: vec![],
        core_types: BTreeSet::new(),
    };
    let mut selected = BTreeMap::new();
    for node in nodes {
        let parts = node.list().unwrap();
        if parts[1].text() != Some("wlt:std") {
            return Err(Error::at(
                source,
                parts[1].span,
                "unknown standard module; expected wlt:std",
            )
            .with_code("E_STD_MODULE"));
        }
        if parts.len() < 3 {
            return Err(
                Error::at(source, node.span, "wlt:std requires explicit imports")
                    .with_code("E_STD_MEMBER"),
            );
        }
        for item in &parts[2..] {
            let fields = item.list().unwrap_or(&[]);
            if !(2..=3).contains(&fields.len())
                || !matches!(item.head(), Some("func" | "type" | "tag"))
                || fields[1..].iter().any(|field| field.atom().is_none())
            {
                return Err(
                    Error::at(source, item.span, "expected (type/func $member [$alias])")
                        .with_code("E_STD_MEMBER"),
                );
            }
            let member = (
                item.head().unwrap().to_owned(),
                fields[1].atom().unwrap().to_owned(),
            );
            if available_types.contains(&member) {
                libraries.core_types.insert(member.1);
                continue;
            }
            let file = catalogue.get(&member).ok_or_else(|| {
                Error::at(
                    source,
                    item.span,
                    format!("wlt:std does not export {} {}", member.0, member.1),
                )
                .with_code("E_STD_MEMBER")
            })?;
            let unit = libraries.load(file)?;
            selected.insert(member, unit);
        }
    }
    let mut facade = String::from("(%export");
    for name in &libraries.core_types {
        facade.push_str(&format!(" (type {name})"));
    }
    facade.push(')');
    for declaration in &types.list().unwrap()[1..] {
        if declaration.head() == Some("%type.alias")
            && declaration
                .list()
                .and_then(|p| p.get(1))
                .and_then(Sexp::atom)
                .is_some_and(|name| libraries.core_types.contains(name))
        {
            facade.push_str(&TYPES[declaration.span.start..declaration.span.end]);
        }
    }
    for ((kind, name), unit) in selected {
        facade.push_str(&format!(
            "\n(%import {unit} ({kind} {name}))\n(%export ({kind} {name}))\n"
        ));
    }
    let extra = format!(
        "\n(%file.namespace {prefix} {facade})\n{}",
        libraries.units.join("")
    );
    let mut text = source.to_owned();
    text.insert_str(result.insert, &extra);
    result.line_offset = source[..result.insert]
        .bytes()
        .filter(|b| *b == b'\n')
        .count();
    result.namespace = prefix;
    result.text = Some(text);
    Ok(result)
}
