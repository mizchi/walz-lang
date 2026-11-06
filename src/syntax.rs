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
            | "effect"
            | "namespace"
            | "file.namespace"
            | "file.use"
            | "legacy.namespace"
            | "public"
            | "use"
            | "include"
            | "type.alias"
            | "type.params"
            | "where"
            | "test"
            | "bench"
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

/// Historical spelling edits retain a mapping to the original source offsets.
pub(crate) struct Migration {
    text: String,
    original: String,
    changes: Vec<(Span, usize)>,
}
impl Migration {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
    pub(crate) fn original(&self) -> &str {
        &self.original
    }
    pub(crate) fn original_offset(&self, offset: usize) -> usize {
        let mut original_cursor = 0;
        let mut new_cursor = 0;
        for &(span, len) in &self.changes {
            let new_start = new_cursor + span.start - original_cursor;
            if offset < new_start {
                return original_cursor + offset - new_cursor;
            }
            if offset < new_start + len {
                return span.start + (offset - new_start).min(span.end - span.start);
            }
            original_cursor = span.end;
            new_cursor = new_start + len;
        }
        original_cursor + offset - new_cursor
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
            changes: vec![],
        });
    }
    let mut root = crate::sexpr::parse(source)?;
    let mut plan = Plan::new(source, true);
    let mut pending = vec![&root];
    while let Some(node) = pending.pop() {
        if matches!(node.atom(), Some("$json.Object" | "$json.new"))
            || matches!(node.head(), Some("namespace" | "%namespace"))
                && node.list().and_then(|p| p.get(1)).and_then(Sexp::atom) == Some("$json")
        {
            plan.old_json_object = true;
            break;
        }
        if let Some(items) = node.list() {
            pending.extend(items);
        }
    }
    plan.walk(&mut root, false, 0)?;
    plan.types(&mut root)?;
    let mut edits = plan
        .edits
        .into_iter()
        .map(|(start, value)| (Span { start, end: start }, value))
        .chain(plan.replacements)
        .collect::<Vec<_>>();
    edits.sort_by_key(|(span, _)| *span);
    let mut text = String::new();
    let mut cursor = 0;
    let mut changes = vec![];
    for (span, value) in edits {
        text.push_str(&source[cursor..span.start]);
        text.push_str(&value);
        changes.push((span, value.len()));
        cursor = span.end;
    }
    text.push_str(&source[cursor..]);
    Ok(Migration {
        text,
        original: source.into(),
        changes,
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
    old_json_object: bool,
    marked: BTreeSet<Span>,
    edits: BTreeMap<usize, String>,
    replacements: BTreeMap<Span, String>,
}
impl<'a> Plan<'a> {
    fn new(source: &'a str, legacy: bool) -> Self {
        Self {
            source,
            legacy,
            old_json_object: false,
            marked: BTreeSet::new(),
            edits: BTreeMap::new(),
            replacements: BTreeMap::new(),
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
                .filter(|n| matches!(*n, "text" | "string" | "bytes" | "unit"))
            {
                let core = crate::naming::core(name).into_owned();
                let public = crate::naming::public(&core);
                let prefixed = self.marked.contains(&node.span);
                self.require(node, &public)?;
                if self.legacy && !prefixed && name != public {
                    self.replacements.insert(node.span, public.into_owned());
                }
                node.kind = Kind::Atom(core);
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
        // `(type T)` marks a let annotation at the surface; the checked core
        // and the native macro expander share the existing type operand.
        if matches!(node.head(), Some("let" | "%let"))
            && let Kind::List(items) = &mut node.kind
            && items.len() == 5
            && items[2].head() == Some("type")
        {
            let annotation = items[2].list().unwrap();
            if annotation.len() != 2 {
                return Err(Error::at(
                    self.source,
                    items[2].span,
                    "expected one type in %let annotation: (type T)",
                )
                .with_code("E_LET_TYPE"));
            }
            items[2] = annotation[1].clone();
        }
        if let Kind::Atom(name) = &mut node.kind {
            if let Some(bare) = name.strip_prefix('%') {
                let core = crate::naming::core(bare);
                let public = crate::naming::public(&core);
                if !matches!(core.as_ref(), "text" | "bytes" | "unit")
                    || !self.legacy && bare != public
                {
                    return Err(Error::at(
                        self.source,
                        node.span,
                        format!("unknown WLT type `%{bare}`"),
                    )
                    .with_code("E_WLT_EXTENSION"));
                }
                if self.legacy && bare != public {
                    self.replacements.insert(node.span, format!("%{public}"));
                }
                self.marked.insert(node.span);
                *name = core.into_owned();
            }
            return Ok(());
        }
        let mut head = node.head().unwrap_or("").to_owned();
        if head.starts_with('@') {
            return Ok(());
        }
        if matches!(head.as_str(), "native.loop" | "native.try" | "struct.named") {
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
        if !self.legacy && matches!(head.as_str(), "namespace" | "%namespace" | "%import") {
            return Err(Error::at(
                self.source,
                items[0].span,
                if head == "%import" {
                    "static source imports use %use"
                } else {
                    "namespace declarations are removed; put the module in a file"
                },
            )
            .with_code("E_WLT_EXTENSION")
            .help("use (%use \"./module.wlt\" (func $member)) to select file exports"));
        }
        if !self.legacy && head == "%use" {
            let path = items.get(1).and_then(Sexp::text);
            if !path.is_some_and(|path| {
                path.starts_with("wlt:") || path.starts_with("./") || path.starts_with("../")
            }) {
                return Err(Error::at(self.source, node.span,
                    "%use requires a package name or an explicit relative path such as ./module.wlt")
                    .with_code("E_USE_PATH"));
            }
        }
        if self.legacy {
            if matches!(head.as_str(), "namespace" | "%namespace") {
                head = "%legacy.namespace".into();
                self.replacements.insert(items[0].span, head.clone());
                items[0].kind = Kind::Atom(head.clone());
            } else if matches!(head.as_str(), "import" | "%import" | "use" | "%use")
                && items.get(2).and_then(Sexp::text).is_none()
            {
                head = if items.get(1).and_then(Sexp::atom).is_some() {
                    "%file.use"
                } else {
                    "%use"
                }
                .into();
                self.replacements.insert(items[0].span, head.clone());
                items[0].kind = Kind::Atom(head.clone());
            }
            let bare = head.trim_start_matches('%');
            let public = if bare == "effects" {
                "effect".into()
            } else {
                crate::naming::public(bare).into_owned()
            };
            let renamed = format!("{}{public}", if head.starts_with('%') { "%" } else { "" });
            if renamed != head {
                head = renamed;
                self.replacements.insert(items[0].span, head.clone());
                items[0].kind = Kind::Atom(head.clone());
            }
        }
        if self.legacy
            && matches!(head.as_str(), "import" | "%import" | "%use")
            && matches!(
                items.get(1).and_then(Sexp::text),
                Some("wlt:std" | "wlt:core")
            )
            && items.get(2).and_then(Sexp::text).is_none()
        {
            if items[1].text() == Some("wlt:std") {
                self.replacements
                    .insert(items[1].span, "\"wlt:core\"".into());
                items[1].kind = Kind::Text("wlt:core".into());
            }
            for binding in &items[2..] {
                if let Some(kind @ ("type" | "func")) = binding.head()
                    && let Some(fields) = binding.list()
                    && let Some(old) = fields.get(1).and_then(Sexp::atom)
                {
                    let new = crate::naming::legacy_member(kind, old, self.old_json_object);
                    if new != old {
                        self.replacements.insert(fields[1].span, new.into_owned());
                        if fields.len() == 2 {
                            self.edits.insert(fields[1].span.end, format!(" {old}"));
                        }
                    }
                }
            }
        }
        if head == "%inst" {
            if items.len() < 2 || !items[1].atom().is_some_and(|n| n.starts_with('$')) {
                return Err(
                    Error::at(self.source, node.span, "expected (%inst $name types...)")
                        .with_code("E_WLT_EXTENSION"),
                );
            }
            items.remove(0);
        } else if let Some(bare) = head.strip_prefix('%') {
            let core = crate::naming::core(bare);
            if crate::naming::public(&core) != bare
                || !entry(&core)
                    && !matches!(bare, "export" | "loop" | "try" | "result" | "struct.new")
            {
                return Err(Error::at(
                    self.source,
                    node.span,
                    format!("unknown WLT extension `%{bare}`"),
                )
                .with_code("E_WLT_EXTENSION"));
            }
            self.marked.insert(items[0].span);
            items[0].kind = Kind::Atom(
                if bare == "file.use" {
                    "use"
                } else if bare == "legacy.namespace" {
                    "namespace"
                } else if bare == "struct.new" {
                    "struct.named"
                } else {
                    core.as_ref()
                }
                .into(),
            );
        } else if head.starts_with('$') {
            if !self.legacy {
                return Err(prefix_error(self.source, node, "inst"));
            }
            self.edits
                .entry(items[0].span.start)
                .or_insert_with(|| "%inst ".into());
        } else if entry(&crate::naming::core(&head)) {
            let core = crate::naming::core(&head);
            self.require(&items[0], &crate::naming::public(&core))?;
            items[0].kind = Kind::Atom(core.into_owned());
        } else if head == "import" && items.get(2).and_then(Sexp::text).is_none() {
            self.require(&items[0], "use")?;
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
            if normalized == "struct.named" && i >= 2 {
                if let Kind::List(binding) = &mut child.kind {
                    for expression in binding.iter_mut().skip(1) {
                        self.walk(expression, false, depth + 1)?;
                    }
                }
                continue;
            }
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
