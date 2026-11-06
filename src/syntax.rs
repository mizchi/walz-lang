//! Explicit WLT surface syntax. The checked implementation IR keeps its own
//! vocabulary; only recognized `%` entries may introduce those extensions.
use crate::{
    Error,
    ir::Span,
    sexpr::{Kind, Sexp},
};
use std::collections::{BTreeMap, BTreeSet};

fn entry(name: &str) -> bool {
    matches!(
        name,
        "let"
            | "newtype"
            | "for"
            | "match"
            | "recur"
            | "closure"
            | "capture"
            | "effects"
            | "namespace"
            | "file.namespace"
            | "public"
            | "use"
            | "include"
            | "type.alias"
            | "type.params"
            | "where"
            | "test"
            | "law"
            | "move"
            | "using"
            | "variant"
            | "resource"
            | "borrow"
            | "list"
            | "option"
            | "pair"
            | "own"
            | "future"
            | "stream"
    ) || [
        "closure.",
        "newtype.",
        "list.",
        "bytes.",
        "text.",
        "option.",
        "pair.",
        "result.",
        "own.",
        "future.",
        "stream.",
        "resource.",
        "variant.",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}

fn prefix_error(source: &str, node: &Sexp, name: &str) -> Error {
    Error::at(
        source,
        node.span,
        format!("WLT extension `{name}` requires `%{name}`"),
    )
    .with_code("E_WLT_PREFIX")
    .help(format!("write `%{name}` for this WLT extension"))
}

/// An insertion-only migration preserves comments, strings and identifiers.
pub(crate) struct Migration {
    text: String,
    original: String,
    insertions: Vec<(usize, usize)>,
}
impl Migration {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
    pub(crate) fn original(&self) -> &str {
        &self.original
    }
    pub(crate) fn original_offset(&self, offset: usize) -> usize {
        let mut added = 0;
        for &(start, len) in &self.insertions {
            let new_start = start + added;
            if offset < new_start {
                break;
            }
            if offset < new_start + len {
                return start;
            }
            added += len;
        }
        offset - added
    }
}

fn native(source: &str) -> bool {
    wat::parse_str(source).is_ok_and(|bytes| {
        wasmparser::Parser::is_core_wasm(&bytes)
            && wasmparser::Validator::new().validate_all(&bytes).is_ok()
    })
}

fn legacy_control(node: &Sexp, head: &str, legacy_try: bool) -> bool {
    let parts = node.list().unwrap();
    if head == "loop" && parts.iter().any(|n| n.head() == Some("bind")) {
        return true;
    }
    let mut pending = parts.iter().skip(1).collect::<Vec<_>>();
    while let Some(n) = pending.pop() {
        if head == "loop" && matches!(n.head(), Some("recur" | "%recur")) {
            return true;
        }
        if head == "try" && legacy_try && n.head() == Some("catch_all") {
            return true;
        }
        if head == "try" && matches!(n.head(), Some("catch" | "catch_all")) {
            let p = n.list().unwrap();
            let binder = p.get(if n.head() == Some("catch") { 2 } else { 1 });
            if binder.and_then(Sexp::list).is_some_and(|xs| {
                xs.iter()
                    .all(|n| n.atom().is_some_and(|s| s.starts_with('$')))
            }) {
                return true;
            }
        }
        if let Some(items) = n.list() {
            pending.extend(items);
        }
    }
    false
}

/// Internal adapter for frozen historical evaluation inputs.
pub(crate) fn convert_legacy(source: &str) -> Result<Migration, Error> {
    if native(source) {
        return Ok(Migration {
            text: source.into(),
            original: source.into(),
            insertions: vec![],
        });
    }
    let mut root = crate::sexpr::parse(source)?;
    let mut plan = Plan::new(source, true);
    plan.walk(&mut root, false, 0)?;
    plan.types(&mut root)?;
    let mut text = String::new();
    let mut cursor = 0;
    let mut insertions = vec![];
    for (offset, value) in plan.edits {
        text.push_str(&source[cursor..offset]);
        text.push_str(&value);
        insertions.push((offset, value.len()));
        cursor = offset;
    }
    text.push_str(&source[cursor..]);
    Ok(Migration {
        text,
        original: source.into(),
        insertions,
    })
}

pub(crate) fn normalize(source: &str, root: &Sexp) -> Result<Sexp, Error> {
    let mut root = root.clone();
    let mut plan = Plan::new(source, false);
    plan.walk(&mut root, false, 0)?;
    plan.types(&mut root)?;
    Ok(root)
}

/// A binding group has exactly the semantics of left-to-right nested lets.
/// Lower it before name, type, effect and ownership analysis so those contracts
/// use the same implementation for both spellings.
fn lower_let_group(source: &str, node: &mut Sexp) -> Result<(), Error> {
    if !matches!(node.head(), Some("let" | "%let")) {
        return Ok(());
    }
    let parts = node.list().unwrap();
    if parts.len() != 3 || parts[1].list().is_none() {
        return Ok(());
    }
    let bindings = parts[1].list().unwrap();
    let error = |span, message| Error::at(source, span, message).with_code("E_LET_BINDINGS");
    if bindings.len() > 256 {
        return Err(error(
            node.span,
            "let binding group expansion limit exceeded",
        ));
    }
    if bindings.is_empty() {
        return Err(error(
            parts[1].span,
            "expected at least one ($name [type] initial) binding",
        ));
    }
    for binding in bindings {
        if binding.list().is_none_or(|p| {
            !(2..=3).contains(&p.len()) || !p[0].atom().is_some_and(|n| n.starts_with('$'))
        }) {
            return Err(error(
                binding.span,
                "expected ($name [type] initial) in %let binding group",
            ));
        }
    }
    // Move the body and initializers; never clone a potentially large subtree.
    let Kind::List(mut parts) = std::mem::replace(&mut node.kind, Kind::List(vec![])) else {
        unreachable!()
    };
    let mut body = parts.pop().unwrap();
    let Kind::List(bindings) = parts.pop().unwrap().kind else {
        unreachable!()
    };
    let head = parts.pop().unwrap();
    for binding in bindings.into_iter().rev() {
        let Kind::List(mut fields) = binding.kind else {
            unreachable!()
        };
        let mut items = vec![head.clone()];
        items.append(&mut fields);
        let span = Span {
            start: binding.span.start,
            end: body.span.end,
        };
        items.push(body);
        body = Sexp {
            kind: Kind::List(items),
            span,
        };
    }
    body.span = node.span;
    *node = body;
    Ok(())
}

struct Plan<'a> {
    source: &'a str,
    legacy: bool,
    marked: BTreeSet<Span>,
    edits: BTreeMap<usize, String>,
}
impl<'a> Plan<'a> {
    fn new(source: &'a str, legacy: bool) -> Self {
        Self {
            source,
            legacy,
            marked: BTreeSet::new(),
            edits: BTreeMap::new(),
        }
    }
    fn require(&mut self, node: &Sexp, name: &str) -> Result<(), Error> {
        if self.marked.contains(&node.span) {
            return Ok(());
        }
        if !self.legacy {
            return Err(prefix_error(self.source, node, name));
        }
        self.edits
            .entry(node.span.start)
            .or_insert_with(|| "%".into());
        self.marked.insert(node.span);
        Ok(())
    }
    fn types(&mut self, root: &mut Sexp) -> Result<(), Error> {
        crate::type_syntax::map_types(root, &mut |node| {
            if let Some(name) = node
                .atom()
                .filter(|n| matches!(*n, "text" | "bytes" | "unit"))
            {
                self.require(node, name)?;
            } else if node.head() == Some("result") {
                let first = &node.list().unwrap()[0];
                self.require(first, "result")?;
            }
            Ok(())
        })
    }
    fn walk(&mut self, node: &mut Sexp, loop_header: bool, depth: usize) -> Result<(), Error> {
        if depth > 256 {
            return Err(
                Error::at(self.source, node.span, "macro expansion limit exceeded")
                    .with_code("E_LET_BINDINGS"),
            );
        }
        lower_let_group(self.source, node)?;
        if let Kind::Atom(name) = &mut node.kind {
            if let Some(bare) = name.strip_prefix('%') {
                if !matches!(bare, "text" | "bytes" | "unit") {
                    return Err(Error::at(
                        self.source,
                        node.span,
                        format!("unknown WLT type `%{bare}`"),
                    )
                    .with_code("E_WLT_EXTENSION"));
                }
                self.marked.insert(node.span);
                *name = bare.into();
            }
            return Ok(());
        }
        let head = node.head().unwrap_or("").to_owned();
        if head.starts_with('@') {
            return Ok(());
        }
        if matches!(head.as_str(), "native.loop" | "native.try") {
            return Err(Error::at(
                self.source,
                node.span,
                "internal syntax is not a source instruction",
            )
            .with_code("E_WLT_EXTENSION"));
        }
        if !self.legacy && head == "try" && legacy_control(node, &head, false) {
            return Err(prefix_error(self.source, node, "try"));
        }
        let old_control = self.legacy
            && matches!(head.as_str(), "loop" | "try")
            && legacy_control(node, &head, self.legacy);
        let Kind::List(items) = &mut node.kind else {
            return Ok(());
        };
        if head == "%inst" {
            if items.len() < 2 || !items[1].atom().is_some_and(|n| n.starts_with('$')) {
                return Err(
                    Error::at(self.source, node.span, "expected (%inst $name types...)")
                        .with_code("E_WLT_EXTENSION"),
                );
            }
            items.remove(0);
        } else if let Some(bare) = head.strip_prefix('%') {
            if !entry(bare) && !matches!(bare, "import" | "export" | "loop" | "try" | "result") {
                return Err(Error::at(
                    self.source,
                    node.span,
                    format!("unknown WLT extension `%{bare}`"),
                )
                .with_code("E_WLT_EXTENSION"));
            }
            self.marked.insert(items[0].span);
            items[0].kind = Kind::Atom(bare.into());
        } else if head.starts_with('$') {
            if !self.legacy {
                return Err(prefix_error(self.source, node, "inst"));
            }
            self.edits
                .entry(items[0].span.start)
                .or_insert_with(|| "%inst ".into());
        } else if entry(&head) {
            self.require(&items[0], &head)?;
        } else if head == "import" && items.get(2).and_then(Sexp::text).is_none() {
            self.require(&items[0], "import")?;
        } else if head == "export" && items.get(1).and_then(Sexp::list).is_some() {
            self.require(&items[0], "export")?;
        } else if matches!(head.as_str(), "loop" | "try") {
            if old_control {
                self.require(&items[0], &head)?;
            } else {
                items[0].kind = Kind::Atom(format!("native.{head}"));
            }
        } else if head == "bind" && !loop_header {
            return Err(prefix_error(self.source, &items[0], "loop"));
        }
        let normalized = items.first().and_then(Sexp::atom).unwrap_or("").to_owned();
        let count = items.len();
        for (i, child) in items.iter_mut().enumerate().skip(1) {
            if normalized == "result.do" && i + 1 < count {
                if let Kind::List(binding) = &mut child.kind {
                    for expression in binding.iter_mut().skip(1) {
                        self.walk(expression, false, depth + 1)?;
                    }
                }
                continue;
            }
            let binders = matches!(normalized.as_str(), "let" | "result.let") && i == 1
                || matches!(normalized.as_str(), "case" | "catch") && i == 2
                || normalized == "stream.fold-result" && i == if count == 8 { 6 } else { 3 };
            if !binders {
                self.walk(
                    child,
                    matches!(normalized.as_str(), "loop" | "for"),
                    depth + 1,
                )?;
            }
        }
        Ok(())
    }
}
