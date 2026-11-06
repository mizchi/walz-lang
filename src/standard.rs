//! Embedded `wlt:core`: ordinary static aliases and library modules. Resolving
//! this package never performs filesystem or host I/O and adds no runtime.
use crate::{
    Error,
    sexpr::{Kind, Sexp},
};
use std::collections::{BTreeMap, BTreeSet};

use crate::standard_sources::{INTERNAL_LIBRARIES, LIBRARIES, TYPES};

fn imports<'a>(node: &'a Sexp, out: &mut Vec<&'a Sexp>) {
    if node.head() == Some("%use") && node.list().unwrap().get(2).and_then(Sexp::text).is_none() {
        out.push(node);
    } else if matches!(
        node.head(),
        Some("module" | "%namespace" | "%file.namespace" | "%legacy.namespace")
    ) {
        for child in node.list().unwrap().iter().skip(1) {
            imports(child, out);
        }
    }
}
mod index {
    include!(concat!(env!("OUT_DIR"), "/standard-index.rs"));
}

struct ParsedLibrary {
    syntax: Sexp,
    body: std::ops::Range<usize>,
    source: &'static str,
}
impl ParsedLibrary {
    fn new(source: &'static str) -> Result<Self, Error> {
        let syntax = crate::sexpr::parse(source)?;
        let parts = syntax
            .list()
            .filter(|_| syntax.head() == Some("module"))
            .ok_or_else(|| Error::message("invalid embedded standard module".into()))?;
        let body = parts[0].span.end..syntax.span.end - 1;
        Ok(Self {
            syntax,
            body,
            source,
        })
    }
}

fn parsed_library(file: &str) -> Result<&'static ParsedLibrary, Error> {
    use std::sync::OnceLock;
    const COUNT: usize = LIBRARIES.len() + INTERNAL_LIBRARIES.len();
    static PARSED: [OnceLock<Result<ParsedLibrary, Error>>; COUNT] =
        [const { OnceLock::new() }; COUNT];
    let (index, (_, source)) = LIBRARIES
        .iter()
        .chain(INTERNAL_LIBRARIES)
        .enumerate()
        .find(|(_, (name, _))| *name == file)
        .ok_or_else(|| Error::message(format!("unknown embedded standard module {file}")))?;
    PARSED[index]
        .get_or_init(|| ParsedLibrary::new(source))
        .as_ref()
        .map_err(Clone::clone)
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
        let parsed = parsed_library(file)?;
        let name = format!("{}-{}", self.prefix, file.trim_end_matches(".wlt"));
        self.active.insert(file.into());
        let mut nodes = vec![];
        imports(&parsed.syntax, &mut nodes);
        let mut edits = vec![];
        for node in nodes {
            let path = &node.list().unwrap()[1];
            let target = if path.text() == Some("wlt:core") {
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
                        .and_then(|path| path.strip_prefix("./"))
                        .ok_or_else(|| {
                            Error::message("embedded dependencies require ./ relative paths".into())
                        })?,
                )?
            };
            edits.push((path.span, target));
            edits.push((node.list().unwrap()[0].span, "%file.use".into()));
        }
        let mut text = parsed.source[parsed.body.clone()].to_owned();
        edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
        for (span, name) in edits {
            text.replace_range(
                span.start - parsed.body.start..span.end - parsed.body.start,
                &name,
            );
        }
        self.units
            .push(format!("\n(%file.namespace {name} {})\n", text));
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
    pub fn syntax(&self, original: Sexp) -> Result<Sexp, Error> {
        if let Some(source) = &self.text {
            drop(original);
            crate::sexpr::parse(source)
        } else {
            Ok(original)
        }
    }
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
            if child.head() == Some("use")
                && child.list().unwrap().get(1).and_then(Sexp::text) == Some("wlt:core")
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
                    label.location.path = Some("wlt:core".into());
                    label.location.line = label.location.line.saturating_sub(self.line_offset);
                }
            }
        }
        error
    }
}

pub(crate) fn prepare(source: &str, root: &Sexp) -> Result<Prepared, Error> {
    let mut nodes = vec![];
    imports(root, &mut nodes);
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
    let mut pending = vec![root];
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
        let prefix = format!("$wlt-core-{next}");
        next += 1;
        if !names.iter().any(|n| n.starts_with(prefix.as_str())) {
            break prefix;
        }
    };
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
        if parts[1].text() != Some("wlt:core") {
            return Err(Error::at(
                source,
                parts[1].span,
                "unknown built-in module; expected wlt:core",
            )
            .with_code("E_CORE_MODULE"));
        }
        if parts.len() < 3 {
            return Err(
                Error::at(source, node.span, "wlt:core requires explicit imports")
                    .with_code("E_CORE_MEMBER"),
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
                        .with_code("E_CORE_MEMBER"),
                );
            }
            let member = (
                item.head().unwrap().to_owned(),
                fields[1].atom().unwrap().to_owned(),
            );
            if member.0 == "type"
                && index::CORE_ALIASES
                    .iter()
                    .any(|(name, _, _)| *name == member.1)
            {
                libraries.core_types.insert(member.1);
                continue;
            }
            let file = index::MEMBERS
                .binary_search_by(|&(kind, name, _)| {
                    (kind, name).cmp(&(member.0.as_str(), member.1.as_str()))
                })
                .ok()
                .map(|position| index::MEMBERS[position].2)
                .ok_or_else(|| {
                    Error::at(
                        source,
                        item.span,
                        format!("wlt:core does not export {} {}", member.0, member.1),
                    )
                    .with_code("E_CORE_MEMBER")
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
    for (name, start, end) in index::CORE_ALIASES {
        if libraries.core_types.contains(*name) {
            facade.push_str(&TYPES[*start..*end]);
        }
    }
    for ((kind, name), unit) in selected {
        facade.push_str(&format!(
            "\n(%file.use {unit} ({kind} {name}))\n(%export ({kind} {name}))\n"
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

#[cfg(test)]
mod performance_contracts {
    #[test]
    fn generated_index_matches_source_exports_and_alias_ranges() {
        use super::*;
        let mut expected = BTreeMap::new();
        for (file, source) in LIBRARIES {
            let mut members = BTreeSet::new();
            crate::standard_catalogue::exports(
                &crate::sexpr::parse(source).unwrap(),
                "",
                &mut members,
            );
            for (kind, name) in members {
                expected.insert((kind, name), *file);
            }
        }
        let actual = index::MEMBERS
            .iter()
            .map(|&(kind, name, file)| ((kind.to_owned(), name.to_owned()), file))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected);
        assert!(
            index::MEMBERS
                .windows(2)
                .all(|pair| (pair[0].0, pair[0].1) < (pair[1].0, pair[1].1))
        );
        let root = crate::sexpr::parse(TYPES).unwrap();
        let mut exported_types = BTreeSet::new();
        crate::standard_catalogue::exports(&root, "", &mut exported_types);
        let expected = root
            .list()
            .unwrap()
            .iter()
            .filter(|n| n.head() == Some("%type.alias"))
            .filter(|n| {
                exported_types.contains(&(
                    "type".to_owned(),
                    n.list().unwrap()[1].atom().unwrap().to_owned(),
                ))
            })
            .map(|n| {
                (
                    n.list().unwrap()[1].atom().unwrap(),
                    n.span.start,
                    n.span.end,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(index::CORE_ALIASES, expected);
    }

    #[test]
    fn type_only_imports_do_not_parse_library_implementations() {
        crate::sexpr::take_parse_log();
        crate::check(
            r#"(module (%use "wlt:core" (type $result))
          (func $main (result (%inst $result i64 i32)) (%result.ok 42)))"#,
        )
        .unwrap();
        let calls = crate::sexpr::take_parse_log();
        for (name, text) in crate::standard_sources::LIBRARIES {
            assert!(
                !calls.contains(&crate::sexpr::parse_key(text)),
                "parsed unused library {name}"
            );
        }
    }

    #[test]
    fn parsing_cached_templates_does_not_share_import_aliases_or_hygiene() {
        let workers = (0..8)
            .map(|index| {
                std::thread::spawn(move || {
                    let source = format!(
                        r#"(module
              (%use "wlt:core" (type $string) (func $string.concat $join{index}))
              (func $wlt-core-{index} (result i32) (i32.const 0))
              (func $main (export "main") (param $value $string) (result $string)
                (call $join{index} $value $value)))"#
                    );
                    for _ in 0..4 {
                        let module = crate::check(&source).unwrap();
                        assert!(
                            module
                                .functions
                                .iter()
                                .any(|f| f.name == format!("$wlt-core-{index}"))
                        );
                        let error = crate::check(
                            &source.replace(&format!("(call $join{index}"), "(call $missing"),
                        )
                        .unwrap_err();
                        assert_eq!(error.diagnostic().code, "E_CHECK");
                        assert!(
                            error.to_string().contains("unknown func `$missing`"),
                            "{error}"
                        );
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
    }
}
