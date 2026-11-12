//! Current-revision declaration/reference index. Never supplies checked types
//! or executable IR; it remains useful when a function body fails validation.
use super::*;
use std::collections::BTreeSet;
const MAX_ENTRIES: usize = 32_768;

#[derive(Clone)]
struct Symbol {
    name: String,
    detail: String,
    declaration: Option<Label>,
    visibility: Span,
    kind: u32,
}
struct Reference {
    span: Span,
    symbol: Symbol,
}
#[derive(Default)]
pub(super) struct Index {
    references: Vec<Reference>,
    symbols: Vec<Symbol>,
    macro_bindings: Vec<(Sexp, Sexp)>,
}
fn head(n: &Sexp) -> Option<&str> {
    n.head().map(|s| s.trim_start_matches('@'))
}
fn snippet(text: &str, n: &Sexp) -> String {
    text.get(n.span.start..n.span.end)
        .unwrap_or("")
        .chars()
        .take(256)
        .collect()
}
fn short(text: String) -> String {
    text.chars().take(512).collect()
}
impl Index {
    pub fn new(source: &Source, raw: &Sexp, resolve: bool) -> Self {
        let prepared = (|| {
            if !resolve {
                return None;
            }
            let standard = crate::standard::prepare(source.text(), raw).ok()?;
            let text = standard.source(source.text()).to_owned();
            let syntax = standard.syntax(raw.clone()).ok()?;
            let mut syntax = crate::syntax::normalize(&text, &syntax).ok()?;
            standard.rewrite(&mut syntax);
            let modules = crate::modules::expand(&text, &syntax).ok()?;
            let syntax = crate::generics::expand(&text, &modules.syntax)
                .map(|g| g.syntax)
                .unwrap_or(modules.syntax);
            Some((text, syntax))
        })();
        let (text, syntax) = prepared
            .as_ref()
            .map(|(s, n)| (s.as_str(), n))
            .unwrap_or((source.text(), raw));
        let mut index = Self::default();
        let mut macros = BTreeMap::new();
        let mut pending = vec![syntax];
        while let Some(n) = pending.pop() {
            if matches!(head(n), Some("macro")) {
                if let Ok(m) = crate::macros::declaration(text, n)
                    && let Some(name) = n.list().and_then(|p| p.get(1)).and_then(Sexp::atom)
                {
                    macros.insert(name.to_owned(), m);
                }
            } else if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        let mut pending = vec![syntax];
        while let Some(n) = pending.pop() {
            if let Some(m) = n
                .head()
                .and_then(|h| h.strip_prefix('@'))
                .and_then(|n| macros.get(n))
            {
                index.macro_bindings.extend(m.editor_bindings(n));
            }
            if head(n) != Some("macro")
                && let Some(p) = n.list()
            {
                pending.extend(p);
            }
        }
        let mut globals = BTreeMap::new();
        index.declarations(source, text, syntax, "", syntax.span, &mut globals);
        index.bodies(source, text, syntax, "", &globals);
        index.imports(source, raw, raw.span);
        index
    }
    fn symbol(
        source: &Source,
        text: &str,
        name: &Sexp,
        detail: String,
        visibility: Span,
        kind: u32,
    ) -> Option<Symbol> {
        let identifier = snippet(text, name);
        if name.atom().is_none_or(|n| n.len() > 256)
            || identifier.len() > 256
            || !identifier.starts_with('$')
        {
            return None;
        }
        Some(Symbol {
            name: identifier,
            detail: short(detail),
            declaration: source.label(name.span, "declaration"),
            visibility,
            kind,
        })
    }
    fn declarations(
        &mut self,
        source: &Source,
        text: &str,
        n: &Sexp,
        scope: &str,
        visibility: Span,
        out: &mut BTreeMap<(String, String, String), Symbol>,
    ) {
        if out.len() >= 4096 {
            return;
        }
        let Some(p) = n.list() else {
            return;
        };
        match head(n) {
            Some("module") => {
                for c in &p[1..] {
                    self.declarations(source, text, c, scope, n.span, out)
                }
            }
            Some("file.namespace" | "namespace") => {
                if let Some(name) = p.get(1).and_then(Sexp::atom) {
                    for c in &p[2..] {
                        self.declarations(source, text, c, name, n.span, out)
                    }
                }
            }
            Some("import") => {
                if let Some(c) = p.last() {
                    self.declarations(source, text, c, scope, visibility, out)
                }
            }
            Some("func" | "closure" | "type" | "type.alias" | "tag" | "memory" | "macro") => {
                let Some(name) = p.get(1).filter(|n| n.atom().is_some()) else {
                    return;
                };
                let identifier = snippet(text, name);
                let headers = p[2..]
                    .iter()
                    .filter(|n| {
                        matches!(
                            head(n),
                            Some("param" | "result" | "type.params" | "where" | "rule")
                        )
                    })
                    .map(|n| {
                        snippet(
                            text,
                            if head(n) == Some("rule") {
                                n.list().and_then(|p| p.get(1)).unwrap_or(n)
                            } else {
                                n
                            },
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let kind = if matches!(head(n), Some("func" | "closure")) {
                    3
                } else if head(n) == Some("macro") {
                    14
                } else {
                    22
                };
                if let Some(symbol) = Self::symbol(
                    source,
                    text,
                    name,
                    format!(
                        "{}{identifier} {headers}",
                        if head(n) == Some("macro") {
                            "macro "
                        } else {
                            ""
                        }
                    ),
                    visibility,
                    kind,
                ) {
                    let namespace = match head(n) {
                        Some("func" | "closure") => "func",
                        Some("type" | "type.alias") => "type",
                        Some(other) => other,
                        None => return,
                    };
                    out.insert(
                        (scope.into(), namespace.into(), name.atom().unwrap().into()),
                        symbol.clone(),
                    );
                    self.symbols.push(symbol);
                }
            }
            _ => {}
        }
    }
    fn imports(&mut self, source: &Source, n: &Sexp, visibility: Span) {
        if self.symbols.len() >= MAX_ENTRIES {
            return;
        }
        let Some(p) = n.list() else {
            return;
        };
        match head(n) {
            Some("module" | "file.namespace" | "namespace") => {
                for c in &p[1..] {
                    self.imports(source, c, n.span)
                }
            }
            Some("use" | "file.use") => {
                if p.get(1).and_then(Sexp::text) == Some("walz:core") {
                    for (name, kind) in crate::standard::completion_exports() {
                        if self.symbols.len() >= MAX_ENTRIES {
                            break;
                        }
                        self.symbols.push(Symbol {
                            name: name.into(),
                            detail: "walz:core export (explicit import required)".into(),
                            declaration: source.label(n.span, "core import"),
                            visibility: n.span,
                            kind,
                        });
                    }
                }
                for binding in p.iter().skip(2) {
                    if let Some(name) = binding
                        .list()
                        .and_then(|p| p.last())
                        .filter(|n| n.atom().is_some())
                        && let Some(symbol) = Self::symbol(
                            source,
                            source.text(),
                            name,
                            if head(binding) == Some("macro") {
                                "macro import selection"
                            } else {
                                "explicit import selection"
                            }
                            .into(),
                            visibility,
                            if head(binding) == Some("func") {
                                3
                            } else if head(binding) == Some("macro") {
                                14
                            } else {
                                22
                            },
                        )
                    {
                        self.symbols.push(symbol);
                    }
                }
            }
            _ => {}
        }
    }
    fn bodies(
        &mut self,
        source: &Source,
        text: &str,
        n: &Sexp,
        scope: &str,
        globals: &BTreeMap<(String, String, String), Symbol>,
    ) {
        let Some(p) = n.list() else {
            return;
        };
        match head(n) {
            Some("module") => {
                for c in &p[1..] {
                    self.bodies(source, text, c, scope, globals)
                }
            }
            Some("file.namespace" | "namespace") => {
                if let Some(name) = p.get(1).and_then(Sexp::atom) {
                    for c in &p[2..] {
                        self.bodies(source, text, c, name, globals)
                    }
                }
            }
            Some("func" | "closure") => {
                let mut locals = vec![];
                for field in p
                    .iter()
                    .skip(2)
                    .filter(|n| matches!(head(n), Some("param" | "capture" | "local")))
                {
                    if let Some(parts) = field.list()
                        && let Some(name) = parts.get(1)
                    {
                        self.bind(source, text, name, parts.get(2), n.span, &mut locals);
                    }
                }
                for body in p.iter().skip(2).filter(|n| {
                    !matches!(
                        head(n),
                        Some(
                            "param"
                                | "capture"
                                | "local"
                                | "result"
                                | "export"
                                | "effect"
                                | "type.params"
                                | "where"
                        )
                    )
                }) {
                    self.walk(source, text, body, scope, globals, &mut locals);
                }
            }
            _ => {}
        }
    }
    fn bind(
        &mut self,
        source: &Source,
        text: &str,
        name: &Sexp,
        ty: Option<&Sexp>,
        visibility: Span,
        locals: &mut Vec<Symbol>,
    ) {
        if self.symbols.len() >= MAX_ENTRIES {
            return;
        }
        if let Some(parts) = name.list() {
            for part in parts {
                self.bind(source, text, part, None, visibility, locals);
            }
            return;
        }
        let detail = ty
            .map(|n| format!("binding {}: {}", snippet(text, name), snippet(text, n)))
            .unwrap_or_else(|| format!("binding {} (no inferred type)", snippet(text, name)));
        if let Some(symbol) = Self::symbol(source, text, name, detail, visibility, 6) {
            self.reference(name, Some(&symbol));
            self.symbols.push(symbol.clone());
            locals.push(symbol);
        }
    }
    fn reference(&mut self, n: &Sexp, symbol: Option<&Symbol>) {
        if self.references.len() < MAX_ENTRIES
            && let Some(symbol) = symbol
        {
            self.references.push(Reference {
                span: n.span,
                symbol: symbol.clone(),
            });
        }
    }
    fn walk(
        &mut self,
        source: &Source,
        text: &str,
        n: &Sexp,
        scope: &str,
        globals: &BTreeMap<(String, String, String), Symbol>,
        locals: &mut Vec<Symbol>,
    ) {
        if self.references.len() >= MAX_ENTRIES {
            return;
        }
        if let Some(name) = n.atom() {
            let symbol = locals.iter().rev().find(|s| s.name == name);
            self.reference(n, symbol);
            return;
        }
        let Some(p) = n.list() else {
            return;
        };
        let prior = locals.len();
        let binders = self
            .macro_bindings
            .iter()
            .filter(|(_, body)| body.span == n.span)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for binder in binders {
            self.bind(source, text, &binder, None, n.span, locals);
        }
        match head(n) {
            Some(name)
                if name.starts_with('$') && n.head().is_some_and(|h| h.starts_with("@$")) =>
            {
                self.reference(
                    &p[0],
                    globals.get(&(scope.into(), "macro".into(), name.into())),
                );
                for child in &p[1..] {
                    if self
                        .macro_bindings
                        .iter()
                        .any(|(binder, _)| binder.span == child.span)
                    {
                        continue;
                    }
                    self.walk(source, text, child, scope, globals, locals);
                }
            }
            Some("result" | "type" | "type.params" | "where" | "inst") => {}
            Some("let" | "using") => {
                if let Some(bindings) = p.get(1).filter(|_| p.len() == 3).and_then(Sexp::list) {
                    for b in bindings {
                        if let Some(fields) = b.list()
                            && fields.len() >= 2
                        {
                            let value = fields.last().unwrap();
                            self.walk(source, text, value, scope, globals, locals);
                            self.bind(
                                source,
                                text,
                                &fields[0],
                                (fields.len() == 3).then(|| &fields[1]),
                                Span {
                                    start: value.span.end,
                                    end: n.span.end,
                                },
                                locals,
                            );
                        }
                    }
                    if let Some(body) = p.get(2) {
                        self.walk(source, text, body, scope, globals, locals);
                    }
                } else if p.len() >= 4 {
                    let value = &p[p.len() - 2];
                    self.walk(source, text, value, scope, globals, locals);
                    self.bind(
                        source,
                        text,
                        &p[1],
                        (p.len() == 5).then(|| &p[2]),
                        Span {
                            start: value.span.end,
                            end: n.span.end,
                        },
                        locals,
                    );
                    self.walk(source, text, p.last().unwrap(), scope, globals, locals);
                }
            }
            Some("for-each") => {
                if let Some([_, names, value, body]) = n.list() {
                    self.walk(source, text, value, scope, globals, locals);
                    self.bind(source, text, names, None, body.span, locals);
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
            Some("struct.update") => {
                if let Some(ty) = p.get(1) {
                    let name = if head(ty) == Some("inst") {
                        ty.list().and_then(|p| p.get(1))
                    } else {
                        Some(ty)
                    };
                    if let Some(name) = name
                        && let Some(atom) = name.atom()
                    {
                        self.reference(
                            name,
                            globals.get(&(scope.into(), "type".into(), atom.into())),
                        );
                    }
                }
                if let Some(value) = p.get(2) {
                    self.walk(source, text, value, scope, globals, locals);
                }
                if let Some([_, name, body]) = p.get(3).and_then(Sexp::list) {
                    self.bind(source, text, name, None, body.span, locals);
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
            Some("result.do") => {
                for b in p.iter().skip(1).take(p.len().saturating_sub(2)) {
                    if let Some([name, value]) = b.list() {
                        self.walk(source, text, value, scope, globals, locals);
                        self.bind(
                            source,
                            text,
                            name,
                            None,
                            Span {
                                start: value.span.end,
                                end: n.span.end,
                            },
                            locals,
                        );
                    }
                }
                if let Some(body) = p.last() {
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
            Some("case" | "catch") => {
                if let Some(names) = p.get(2) {
                    self.bind(source, text, names, None, n.span, locals);
                }
                for body in p.iter().skip(3) {
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
            Some("loop" | "for") => {
                let bindings = p
                    .iter()
                    .filter_map(|b| {
                        if head(b) == Some("bind") {
                            b.list()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                for b in &bindings {
                    if let Some(value) = b.get(3) {
                        self.walk(source, text, value, scope, globals, locals);
                    }
                }
                for b in bindings {
                    if b.len() >= 4 {
                        self.bind(source, text, &b[1], Some(&b[2]), n.span, locals);
                    }
                }
                for body in p.iter().skip(2).filter(|b| head(b) != Some("bind")) {
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
            Some(
                "call" | "closure.call" | "struct.get" | "struct.set" | "struct.new" | "array.get"
                | "array.set" | "array.new" | "array.new_fixed" | "ref.null" | "ref.cast",
            ) => {
                if let Some(operand) = p.get(1) {
                    let name = if matches!(head(operand), Some("inst")) {
                        operand.list().and_then(|p| p.get(1))
                    } else {
                        Some(operand)
                    };
                    if let Some(name) = name
                        && let Some(atom) = name.atom()
                    {
                        self.reference(
                            name,
                            globals.get(&(
                                scope.into(),
                                if matches!(head(n), Some("call" | "closure.call")) {
                                    "func"
                                } else {
                                    "type"
                                }
                                .into(),
                                atom.into(),
                            )),
                        );
                    }
                }
                let start = if matches!(head(n), Some("struct.get" | "struct.set")) {
                    3
                } else {
                    2
                };
                for body in p.iter().skip(start) {
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
            _ => {
                for body in p.iter().skip(1) {
                    self.walk(source, text, body, scope, globals, locals);
                }
            }
        }
        locals.truncate(prior);
    }
    pub fn hover(&self, offsets: &[usize]) -> Option<&str> {
        self.references
            .iter()
            .find(|r| offsets.iter().any(|o| contains(r.span, *o)))
            .map(|r| r.symbol.detail.as_str())
    }
    pub fn definition(&self, offsets: &[usize]) -> Option<&Label> {
        self.references
            .iter()
            .find(|r| offsets.iter().any(|o| contains(r.span, *o)))
            .and_then(|r| r.symbol.declaration.as_ref())
    }
    pub fn completions(&self, path: &Path, offsets: &[usize]) -> Vec<(String, String, u32)> {
        let mut seen = BTreeSet::new();
        self.symbols
            .iter()
            .filter(|s| {
                s.declaration
                    .as_ref()
                    .and_then(|l| l.location.path.as_deref())
                    == Some(path)
                    && offsets.iter().any(|o| contains(s.visibility, *o))
                    && seen.insert(if s.kind == 14 {
                        format!("@{}", s.name)
                    } else {
                        s.name.clone()
                    })
            })
            .take(1024)
            .map(|s| {
                (
                    if s.kind == 14 {
                        format!("@{}", s.name)
                    } else {
                        s.name.clone()
                    },
                    s.detail.clone(),
                    s.kind,
                )
            })
            .collect()
    }
}
