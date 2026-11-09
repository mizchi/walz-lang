//! Declarative syntax extensions. Expansion produces syntax, never trusted IR.
//! Queries are answered by the ordinary checker; all output is checked again.
mod bindings;
mod hygiene;
mod type_info;
pub(crate) use type_info::Projection;
pub use type_info::TypeInfo;

use crate::{
    Error,
    bindings::BindingPolicy,
    ir::Span,
    sexpr::{Kind, Sexp},
};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

const MAX_EXPANSIONS: usize = 512;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 100_000;
const MAX_RECHECK_NODES: usize = 2_000_000;

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub type_schema: &'static str,
    pub definitions: Vec<Definition>,
    pub expansions: Vec<Expansion>,
    pub checking: Checking,
}
/// Work in this immutable compilation, rather than cross-revision cached types.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Checking {
    pub module_passes: usize,
    pub function_checks: usize,
    pub syntax_node_visits: usize,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            schema: "wlt-macro-expansion-v1",
            type_schema: "wlt-type-info-v1",
            definitions: vec![],
            expansions: vec![],
            checking: Checking::default(),
        }
    }
}
impl Report {
    /// Human view of the same checked report used by the CLI and LSP.
    pub fn render(&self, source: &crate::source::Source) -> String {
        use std::fmt::Write;
        fn location(source: &crate::source::Source, span: Span) -> String {
            let Some(label) = source.label(span, "") else {
                return format!("<generated>:byte{}", span.start);
            };
            let l = &label.location;
            format!(
                "{}:{}:{}",
                l.path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<source>".into()),
                l.line,
                l.column
            )
        }
        let mut out =
            String::from("Syntax occurrences only; runtime evaluation depends on control flow.\n");
        for e in &self.expansions {
            writeln!(
                out,
                "\n{} at {} (definition {})",
                e.name,
                location(source, e.call),
                location(source, e.definition)
            )
            .unwrap();
            for input in &e.inputs {
                let count = input.outputs.len();
                writeln!(
                    out,
                    "  {}: {} ({count} syntax occurrence{}) at {}",
                    input.name,
                    input.usage.name(),
                    if count == 1 { "" } else { "s" },
                    location(source, input.source)
                )
                .unwrap();
            }
            for (name, info) in &e.types {
                writeln!(out, "  {name}: {}", serde_json::to_string(info).unwrap()).unwrap();
            }
            writeln!(out, "  => {}", e.expanded).unwrap();
        }
        out
    }
    /// Raw spans address the immutable loaded snapshot. Locations additionally
    /// identify the original files, including imported macro definitions.
    pub fn located(&self, source: &crate::source::Source) -> serde_json::Value {
        let mut value = serde_json::to_value(self).expect("serializable macro report");
        for (definition, output) in self
            .definitions
            .iter()
            .zip(value["definitions"].as_array_mut().unwrap())
        {
            output["location"] =
                serde_json::to_value(source.label(definition.declaration, "macro definition"))
                    .unwrap();
        }
        for (expansion, output) in self
            .expansions
            .iter()
            .zip(value["expansions"].as_array_mut().unwrap())
        {
            output["call_location"] =
                serde_json::to_value(source.label(expansion.call, "macro call")).unwrap();
            output["definition_location"] =
                serde_json::to_value(source.label(expansion.definition, "macro definition"))
                    .unwrap();
        }
        value
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Definition {
    pub name: String,
    pub declaration: Span,
    pub rules: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Expansion {
    pub id: usize,
    pub parent: Option<usize>,
    pub name: String,
    pub call: Span,
    pub head: Span,
    pub definition: Span,
    pub rule: Span,
    pub inferred: BTreeMap<String, String>,
    pub types: BTreeMap<String, TypeInfo>,
    pub expanded: String,
    pub nodes: Vec<NodeOrigin>,
    pub inputs: Vec<InputOrigin>,
}
#[derive(Clone, Debug, Serialize)]
pub struct InputOrigin {
    pub name: String,
    pub source: Span,
    /// Empty when syntax is discarded; several IDs when it is duplicated.
    pub outputs: Vec<usize>,
    pub usage: SyntaxUsage,
}
/// Syntactic occurrences in this expansion, not a runtime execution count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyntaxUsage {
    Discarded,
    Single,
    Duplicated,
}
impl SyntaxUsage {
    fn from_count(count: usize) -> Self {
        match count {
            0 => Self::Discarded,
            1 => Self::Single,
            _ => Self::Duplicated,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Discarded => "discarded",
            Self::Single => "single",
            Self::Duplicated => "duplicated",
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct NodeOrigin {
    pub id: usize,
    pub source: Span,
    pub template: Option<Span>,
    pub context: usize,
    pub input: Option<usize>,
}

/// Inspect fully checked macro expansion, without running guest or host code.
pub fn report(source: &str, policy: &BindingPolicy) -> Result<Report, Error> {
    crate::check_observed(source, policy, |_, expanded| expanded.macros.clone())
        .map(|(_, report)| report)
}

pub(crate) fn requested(source: &str) -> bool {
    crate::sexpr::parse(source).is_ok_and(|root| {
        let mut pending = vec![&root];
        while let Some(n) = pending.pop() {
            if matches!(n.head(), Some("macro" | "%macro"))
                || n.head().is_some_and(|h| h.starts_with("%$"))
            {
                return true;
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        false
    })
}

fn query_target(node: &Sexp) -> Option<(Projection, &str)> {
    if let Some(target) = node.atom() {
        return Some((Projection::Identity, target));
    }
    let parts = node.list()?;
    if parts.len() != 2 {
        return None;
    }
    Some((Projection::parse(parts[0].atom()?)?, parts[1].atom()?))
}

#[derive(Clone, Copy, PartialEq)]
enum Category {
    Expr,
    Ident,
    Type,
    Literal,
}
#[derive(Clone)]
struct Variable {
    category: Category,
    repeated: bool,
}
#[derive(Clone)]
pub(crate) struct Rule {
    pattern: Sexp,
    variables: BTreeMap<String, Variable>,
    pub bindings: Vec<(String, String)>,
    infer: Vec<Sexp>,
    guards: Vec<Sexp>,
    template: Sexp,
    span: Span,
}
#[derive(Clone)]
pub(crate) struct Macro {
    name: String,
    declaration: Span,
    pub rules: Vec<Rule>,
}
impl Macro {
    pub fn type_arguments(&self, node: &Sexp) -> Vec<Span> {
        let mut args = node.clone();
        if let Kind::List(p) = &mut args.kind {
            p.remove(0);
        }
        let mut spans = BTreeSet::new();
        for rule in &self.rules {
            let mut captures = Captures::new();
            if !matches(&rule.pattern, &args, false, &mut captures) {
                continue;
            }
            for (name, variable) in &rule.variables {
                if variable.category == Category::Type
                    && let Some(capture) = captures.get(name)
                {
                    match capture {
                        Capture::One(n) => {
                            spans.insert(n.span);
                        }
                        Capture::Many(ns) => {
                            spans.extend(ns.iter().map(|n| n.span));
                        }
                    }
                }
            }
        }
        spans.into_iter().collect()
    }
    /// Editor recovery uses syntax and declared scopes only. If type-directed
    /// alternatives disagree about binding, it does not guess one of them.
    #[cfg(not(target_family = "wasm"))]
    pub fn editor_bindings(&self, node: &Sexp) -> Vec<(Sexp, Sexp)> {
        self.binding_inputs(node).unwrap_or_default()
    }
}
#[derive(Clone)]
pub(crate) enum Capture {
    One(Sexp),
    Many(Vec<Sexp>),
}
impl Capture {
    fn values(&self) -> &[Sexp] {
        match self {
            Self::One(n) => std::slice::from_ref(n),
            Self::Many(ns) => ns,
        }
    }
    fn one(&self) -> Option<&Sexp> {
        if let Self::One(node) = self {
            Some(node)
        } else {
            None
        }
    }
}
pub(crate) type Captures = BTreeMap<String, Capture>;

#[derive(Clone)]
pub(crate) struct Inferred {
    pub syntax: Sexp,
    pub info: TypeInfo,
    pub classes: BTreeSet<&'static str>,
}
pub(crate) struct Request {
    pub node: usize,
    pub output: Sexp,
    expansion: Expansion,
}
#[derive(Default)]
struct State {
    report: Report,
    pending: Option<Request>,
    names: BTreeSet<String>,
    next_name: usize,
    nodes: usize,
    work: usize,
    bytes: usize,
    replacements: BTreeMap<usize, Sexp>,
    next_node: usize,
}
pub(crate) struct Environment {
    pub definitions: BTreeMap<String, Macro>,
    aliases: BTreeMap<String, Sexp>,
    state: RefCell<State>,
}

pub(crate) fn error(source: &str, node: &Sexp, message: impl std::fmt::Display) -> Error {
    Error::at(source, node.span, message).with_code("E_MACRO")
}
fn variable(node: &Sexp) -> Option<(&str, &str)> {
    node.atom()?.strip_prefix('?')?.split_once(':')
}
fn identifier(name: &str) -> bool {
    name.starts_with('$')
        && name.len() > 1
        && !name.starts_with("$__")
        && name[1..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}
fn pattern_variables(
    source: &str,
    node: &Sexp,
    repeated: bool,
    out: &mut BTreeMap<String, Variable>,
) -> Result<(), Error> {
    if let Some((name, category)) = variable(node) {
        let category = match category {
            "expr" => Category::Expr,
            "ident" => Category::Ident,
            "type" => Category::Type,
            "literal" => Category::Literal,
            _ => {
                return Err(error(
                    source,
                    node,
                    "macro fragment must be expr, ident, type, or literal",
                ));
            }
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(error(source, node, "expected ?name:category"));
        }
        if out
            .insert(format!("?{name}"), Variable { category, repeated })
            .is_some()
        {
            return Err(error(source, node, "duplicate macro pattern variable"));
        }
    } else if let Some(parts) = node.list() {
        let rest = parts.last().and_then(Sexp::atom) == Some("...");
        if rest && (repeated || parts.len() < 2) {
            return Err(error(
                source,
                node,
                "only one level of trailing repetition is supported",
            ));
        }
        for (i, child) in parts
            .iter()
            .enumerate()
            .take(parts.len() - usize::from(rest))
        {
            if child.atom() == Some("...") {
                return Err(error(source, child, "repetition must end its list"));
            }
            pattern_variables(source, child, repeated || rest && i + 2 == parts.len(), out)?;
        }
    } else if node.atom().is_some_and(|s| s.starts_with('?')) {
        return Err(error(
            source,
            node,
            "pattern variable needs a fragment category",
        ));
    }
    Ok(())
}

pub(crate) fn declaration(source: &str, node: &Sexp) -> Result<Macro, Error> {
    let parts = node.list().unwrap_or(&[]);
    let name = parts
        .get(1)
        .and_then(Sexp::atom)
        .filter(|s| identifier(s))
        .ok_or_else(|| {
            error(
                source,
                node,
                "expected (%macro $name (rule (pattern...) clauses... template)...)",
            )
        })?;
    if parts.len() < 3 {
        return Err(error(source, node, "macro requires at least one rule"));
    }
    let mut rules = vec![];
    for rule in &parts[2..] {
        let fields = rule.list().unwrap_or(&[]);
        if rule.head() != Some("rule") || fields.len() < 3 || fields[1].list().is_none() {
            return Err(error(
                source,
                rule,
                "expected (rule (pattern...) [infer/when/bind clauses...] template)",
            ));
        }
        let mut variables = BTreeMap::new();
        pattern_variables(source, &fields[1], false, &mut variables)?;
        let mut bindings = vec![];
        let mut infer = vec![];
        let mut guards = vec![];
        let mut available = variables.keys().cloned().collect::<BTreeSet<_>>();
        for clause in &fields[2..fields.len() - 1] {
            let p = clause.list().unwrap_or(&[]);
            match clause.head() {
                Some("bind") if p.len() == 3 => {
                    let binder = p[1].atom().unwrap_or("");
                    let body = p[2].atom().unwrap_or("");
                    if variables
                        .get(binder)
                        .is_none_or(|v| v.category != Category::Ident)
                        || variables
                            .get(body)
                            .is_none_or(|v| v.category != Category::Expr || v.repeated)
                        || bindings.contains(&(binder.into(), body.into()))
                    {
                        return Err(error(
                            source,
                            clause,
                            "bind requires a distinct ident variable and expr body variable",
                        )
                        .with_code("E_MACRO_BINDING"));
                    }
                    bindings.push((binder.into(), body.into()));
                }
                Some("infer") if p.len() > 1 => {
                    for query in &p[1..] {
                        let q = query.list().unwrap_or(&[]);
                        if !(2..=3).contains(&q.len())
                            || !q[0]
                                .atom()
                                .is_some_and(|n| n.starts_with('?') && n.len() > 1)
                            || !query_target(&q[1]).is_some_and(|(_, target)| {
                                target == "expected"
                                    || variables.get(target).is_some_and(|v| {
                                        v.category == Category::Expr && !v.repeated
                                    })
                            })
                        {
                            return Err(error(
                                source,
                                query,
                                "expected (infer (?Type ?expression [expected])...) or (infer (?Type (selector ?expression)))",
                            ));
                        }
                        if q.len() == 3 && q[2].atom() != Some("expected") {
                            return Err(error(source, query, "inference hint must be expected"));
                        }
                        if !available.insert(q[0].atom().unwrap().into()) {
                            return Err(error(source, query, "duplicate inferred type variable"));
                        }
                        infer.push(query.clone());
                    }
                }
                Some("when") if p.len() > 1 => {
                    for guard in &p[1..] {
                        let g = guard.list().unwrap_or(&[]);
                        if !matches!(g.len(), 2 | 3)
                            || !g.get(1).and_then(Sexp::atom).is_some_and(|v| {
                                variables
                                    .get(v)
                                    .is_some_and(|v| v.category == Category::Expr && !v.repeated)
                            })
                            || !(g.len() == 3 && guard.head() == Some("type")
                                || g.len() == 2
                                    && guard
                                        .head()
                                        .and_then(crate::typeclasses::Class::parse)
                                        .is_some())
                        {
                            return Err(error(
                                source,
                                guard,
                                "expected (when (type ?expression Type)...) or (when (Add/Eq/... ?expression)...)",
                            ));
                        }
                        guards.push(guard.clone());
                    }
                }
                _ => {
                    return Err(error(
                        source,
                        clause,
                        "unknown macro clause; expected infer, when, or bind",
                    ));
                }
            }
        }
        let template = fields.last().unwrap().clone();
        let mut pending = vec![&template];
        while let Some(n) = pending.pop() {
            if let Some(name) = n.atom().filter(|n| n.starts_with('?'))
                && !available.contains(name)
            {
                return Err(error(
                    source,
                    n,
                    format!("unknown template variable `{name}`"),
                ));
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        rules.push(Rule {
            pattern: fields[1].clone(),
            variables,
            bindings,
            infer,
            guards,
            template,
            span: rule.span,
        });
    }
    Ok(Macro {
        name: name.into(),
        declaration: parts[1].span,
        rules,
    })
}

fn matches(pattern: &Sexp, input: &Sexp, repeated: bool, captures: &mut Captures) -> bool {
    if let Some((name, category)) = variable(pattern) {
        let valid = match category {
            "expr" => true,
            "ident" => input.atom().is_some_and(identifier),
            "type" => input.atom().is_some() || input.list().is_some(),
            "literal" => {
                input.text().is_some()
                    || matches!(input.kind, Kind::Bytes(_))
                    || input.atom().is_some_and(|n| {
                        let mut offset = 0;
                        wast::lexer::Lexer::new(n)
                            .parse(&mut offset)
                            .ok()
                            .flatten()
                            .is_some_and(|t| {
                                matches!(
                                    t.kind,
                                    wast::lexer::TokenKind::Integer(_)
                                        | wast::lexer::TokenKind::Float(_)
                                )
                            })
                    })
            }
            _ => false,
        };
        if !valid {
            return false;
        }
        let key = format!("?{name}");
        if repeated {
            if let Capture::Many(nodes) =
                captures.entry(key).or_insert_with(|| Capture::Many(vec![]))
            {
                nodes.push(input.clone());
            }
        } else {
            captures.insert(key, Capture::One(input.clone()));
        }
        return true;
    }
    match (&pattern.kind, &input.kind) {
        (Kind::List(p), Kind::List(i)) => {
            let rest = p.last().and_then(Sexp::atom) == Some("...");
            let fixed = p.len().saturating_sub(if rest { 2 } else { 0 });
            if !rest && p.len() != i.len() || rest && i.len() < fixed {
                return false;
            }
            if !p[..fixed]
                .iter()
                .zip(&i[..fixed])
                .all(|(p, i)| matches(p, i, repeated, captures))
            {
                return false;
            }
            !rest
                || i[fixed..]
                    .iter()
                    .all(|n| matches(&p[fixed], n, true, captures))
        }
        _ => key(pattern) == key(input),
    }
}

pub(crate) fn key(node: &Sexp) -> String {
    match &node.kind {
        Kind::Atom(s) => s.clone(),
        Kind::Text(s) => serde_json::to_string(s).unwrap(),
        Kind::Bytes(bytes) => format!(
            "\"{}\"",
            bytes
                .iter()
                .map(|b| format!("\\{b:02x}"))
                .collect::<String>()
        ),
        Kind::List(p) => format!("({})", p.iter().map(key).collect::<Vec<_>>().join(" ")),
    }
}

impl Environment {
    pub fn prepare<'a>(
        source: &str,
        root: &'a Sexp,
    ) -> Result<(Self, std::borrow::Cow<'a, Sexp>), Error> {
        if root
            .list()
            .is_none_or(|p| !p.iter().any(|n| n.head() == Some("macro")))
        {
            return Ok((
                Self {
                    definitions: BTreeMap::new(),
                    aliases: BTreeMap::new(),
                    state: RefCell::default(),
                },
                std::borrow::Cow::Borrowed(root),
            ));
        }
        let mut syntax = root.clone();
        let mut definitions = BTreeMap::new();
        let aliases = root
            .list()
            .unwrap()
            .iter()
            .filter_map(|n| {
                let p = n.list()?;
                (n.head() == Some("type.alias") && p.len() == 3)
                    .then(|| Some((p[1].atom()?.to_owned(), p[2].clone())))?
            })
            .collect();
        let mut state = State::default();
        let mut pending = vec![root];
        while let Some(n) = pending.pop() {
            if let Some(a) = n.atom().filter(|n| n.starts_with('$')) {
                state.names.insert(a.into());
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        if let Kind::List(parts) = &mut syntax.kind {
            for node in parts.iter().skip(1).filter(|n| n.head() == Some("macro")) {
                let definition = declaration(source, node)?;
                state.report.definitions.push(Definition {
                    name: definition.name.clone(),
                    declaration: definition.declaration,
                    rules: definition.rules.iter().map(|r| key(&r.pattern)).collect(),
                });
                if definitions
                    .insert(definition.name.clone(), definition)
                    .is_some()
                {
                    return Err(error(source, node, "duplicate macro"));
                }
            }
            parts.retain(|n| n.head() != Some("macro"));
        }
        Ok((
            Self {
                definitions,
                aliases,
                state: RefCell::new(state),
            },
            std::borrow::Cow::Owned(syntax),
        ))
    }
    pub fn clear_edits(&self) {
        self.state.borrow_mut().replacements.clear();
    }
    pub fn begin_check(&self, root: &Sexp) {
        let mut pending = vec![root];
        let mut state = self.state.borrow_mut();
        state.report.checking.module_passes += 1;
        while let Some(n) = pending.pop() {
            state.next_node = state.next_node.max(n.context.id + 1);
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
    }
    pub fn apply_expansions(&self, root: &mut Sexp) {
        fn walk(n: &mut Sexp, map: &BTreeMap<usize, Sexp>) {
            while let Some(output) = map.get(&n.context.id) {
                *n = output.clone();
            }
            if let Kind::List(p) = &mut n.kind {
                for c in p {
                    walk(c, map);
                }
            }
        }
        walk(root, &self.state.borrow().replacements);
    }
    pub fn replacement(&self, node: &Sexp) -> Option<Sexp> {
        self.state
            .borrow()
            .replacements
            .get(&node.context.id)
            .cloned()
    }
    pub fn finish_function(&self, source: &str, body: &Sexp) -> Result<(), Error> {
        if self.definitions.is_empty() {
            return Ok(());
        }
        let mut expanded = body.clone();
        self.apply_expansions(&mut expanded);
        self.charge_expansion(source, &expanded)
    }
    pub fn inline(
        &self,
        source: &str,
        body: &mut Sexp,
        aliases: &crate::aliases::Expanded,
        inference: &crate::generics::Inference,
        depth: usize,
    ) -> Result<(), Error> {
        let mut request = self.take_pending().expect("pending macro request");
        let mut output = aliases.expression(source, &request.output)?;
        if !inference.resolve_existing(&mut output)? {
            self.state.borrow_mut().pending = Some(request);
            return Err(error(source, body, "macro requires new specialization")
                .with_code("E_MACRO_PENDING"));
        }
        // Resolving a cached application may reveal a concrete transparent alias.
        output = aliases.expression(source, &output)?;
        output = crate::scoped::expand(source, &output)?;
        let mut state = self.state.borrow_mut();
        fn identify(n: &mut Sexp, next: &mut usize, expansion: usize) {
            n.context.id = *next;
            *next += 1;
            if n.context.expansion.is_none() {
                n.context.expansion = Some(expansion);
            }
            if let Kind::List(p) = &mut n.kind {
                for c in p {
                    identify(c, next, expansion);
                }
            }
        }
        identify(&mut output, &mut state.next_node, request.expansion.id);
        state.replacements.insert(request.node, output.clone());
        drop(state);
        request.output = output;
        self.resume_at(source, body, request, depth)
    }
    pub fn function_check(&self, source: &str, body: &Sexp) -> Result<(), Error> {
        self.state.borrow_mut().report.checking.function_checks += 1;
        self.charge_check(source, body)
    }
    pub fn report(&self) -> Report {
        let state = self.state.borrow();
        let mut report = state.report.clone();
        report.checking.syntax_node_visits = state.work;
        report
    }
    pub fn take_pending(&self) -> Option<Request> {
        self.state.borrow_mut().pending.take()
    }

    pub fn request(
        &self,
        source: &str,
        node: &Sexp,
        expected: Option<&Inferred>,
        mut infer: impl FnMut(&Sexp, bool, Projection) -> Result<Inferred, Error>,
    ) -> Result<(), Error> {
        let name = node.head().unwrap_or("").trim_start_matches('%');
        let definition = self.definitions.get(name).ok_or_else(|| {
            error(source, node, format!("unknown macro `{name}`")).with_code("E_MACRO_UNKNOWN")
        })?;
        let mut arguments = node.clone();
        if let Kind::List(p) = &mut arguments.kind {
            p.remove(0);
        }
        let mut rejected = vec![];
        for rule in &definition.rules {
            let mut captures = Captures::new();
            if !matches(&rule.pattern, &arguments, false, &mut captures) {
                continue;
            }
            for (name, v) in &rule.variables {
                if v.repeated {
                    captures
                        .entry(name.clone())
                        .or_insert_with(|| Capture::Many(vec![]));
                }
            }
            let mut inferred = BTreeMap::new();
            let mut type_info = BTreeMap::new();
            let mut types = BTreeMap::new();
            for query in &rule.infer {
                let q = query.list().unwrap();
                let (projection, target) = query_target(&q[1]).unwrap();
                let ty = if target == "expected" {
                    if projection != Projection::Identity {
                        return Err(error(
                            source,
                            query,
                            "project an expression, or infer the expected type directly",
                        )
                        .with_code("E_MACRO_TYPE"));
                    }
                    expected.cloned().ok_or_else(|| {
                        error(
                            source,
                            node,
                            "macro requires an expected result type; add a type annotation",
                        )
                        .with_code("E_MACRO_INFERENCE")
                    })?
                } else {
                    let value = captures[target].one().unwrap();
                    let ty = infer(value, q.len() == 3, projection)?;
                    if projection == Projection::Identity {
                        types.insert(target.to_owned(), ty.clone());
                    }
                    ty
                };
                let name = q[0].atom().unwrap().to_owned();
                type_info.insert(name.clone(), ty.info.clone());
                inferred.insert(name.clone(), key(&ty.syntax));
                captures.insert(name, Capture::One(ty.syntax));
            }
            let mut accepted = true;
            for guard in &rule.guards {
                let g = guard.list().unwrap();
                let value = captures[g[1].atom().unwrap()].one().unwrap();
                let actual = if let Some(ty) = types.get(g[1].atom().unwrap()) {
                    ty.clone()
                } else {
                    let ty = infer(value, false, Projection::Identity)?;
                    types.insert(g[1].atom().unwrap().to_owned(), ty.clone());
                    ty
                };
                if guard.head() == Some("type") {
                    let ty = g[2]
                        .atom()
                        .and_then(|n| captures.get(n))
                        .and_then(Capture::one)
                        .unwrap_or(&g[2]);
                    let required = key(&self.canonical_type(source, ty, 0)?);
                    accepted &= key(&actual.syntax) == required;
                    if !accepted && rejected.len() < 8 {
                        rejected.push(format!(
                            "rule requires `{required}` for {}; inferred `{}`",
                            g[1].atom().unwrap(),
                            key(&actual.syntax)
                        ));
                    }
                } else {
                    accepted &= actual.classes.contains(guard.head().unwrap());
                    if !accepted && rejected.len() < 8 {
                        rejected.push(format!(
                            "rule requires {} for {}; inferred `{}`",
                            guard.head().unwrap(),
                            g[1].atom().unwrap(),
                            key(&actual.syntax)
                        ));
                    }
                }
                type_info.insert(g[1].atom().unwrap().into(), actual.info.clone());
                inferred.insert(g[1].atom().unwrap().into(), key(&actual.syntax));
                if !accepted {
                    break;
                }
            }
            if !accepted {
                continue;
            }
            let mut supplied = BTreeSet::new();
            for (binder, body) in &rule.bindings {
                for captured in captures[binder].values() {
                    let value = captured.atom().unwrap();
                    if !supplied.insert((body, value)) {
                        return Err(error(
                            source,
                            node,
                            "duplicate caller-supplied binders in one body",
                        )
                        .with_code("E_MACRO_BINDING"));
                    }
                }
            }
            let mut state = self.state.borrow_mut();
            let id = state.report.expansions.len();
            let mut parent = node.context.expansion;
            let mut depth = 0;
            while let Some(p) = parent {
                depth += 1;
                parent = state.report.expansions.get(p).and_then(|e| e.parent);
                if depth >= MAX_DEPTH {
                    break;
                }
            }
            if id >= MAX_EXPANSIONS || depth >= MAX_DEPTH {
                return Err(
                    error(source, node, "macro expansion count/depth limit exceeded")
                        .with_code("E_MACRO_LIMIT"),
                );
            }
            let State {
                names, next_name, ..
            } = &mut *state;
            let output = hygiene::instantiate(
                source,
                node,
                rule,
                &captures,
                id,
                (names, next_name),
                &self.definitions,
            )?;
            let mut nodes = vec![];
            let mut pending = vec![&output];
            while let Some(n) = pending.pop() {
                if nodes.len() + state.nodes >= MAX_NODES {
                    return Err(error(source, node, "macro expansion node limit exceeded")
                        .with_code("E_MACRO_LIMIT"));
                }
                nodes.push(NodeOrigin {
                    id: state.nodes + nodes.len(),
                    source: n.span,
                    template: n.context.template,
                    context: n.context.expansion.unwrap_or(id),
                    input: (n.context.id != 0).then_some(n.context.id),
                });
                if let Some(p) = n.list() {
                    pending.extend(p.iter().rev());
                }
            }
            state.nodes += nodes.len();
            let mut inputs = vec![];
            for name in rule.variables.keys() {
                let values = match captures.get(name) {
                    Some(Capture::One(n)) => vec![n],
                    Some(Capture::Many(ns)) => ns.iter().collect(),
                    None => vec![],
                };
                for input in values {
                    let outputs = nodes
                        .iter()
                        .filter(|n| n.input == Some(input.context.id))
                        .map(|n| n.id)
                        .collect::<Vec<_>>();
                    inputs.push(InputOrigin {
                        name: name.clone(),
                        source: input.span,
                        usage: SyntaxUsage::from_count(outputs.len()),
                        outputs,
                    });
                }
            }
            let expanded = key(&output);
            state.bytes += expanded.len();
            if state.bytes > 8 * 1024 * 1024 {
                return Err(error(source, node, "macro expansion text limit exceeded")
                    .with_code("E_MACRO_LIMIT"));
            }
            let expansion = Expansion {
                id,
                parent: node.context.expansion,
                name: name.into(),
                call: node.span,
                head: node.list().unwrap()[0].span,
                definition: definition.declaration,
                rule: rule.span,
                inferred,
                types: type_info,
                expanded,
                nodes,
                inputs,
            };
            state.pending = Some(Request {
                node: node.context.id,
                output,
                expansion,
            });
            return Err(error(source, node, "macro expansion pending").with_code("E_MACRO_PENDING"));
        }
        let mut failure = error(
            source,
            node,
            format!("no matching rule for macro `{name}` (syntax or inferred type constraints)"),
        )
        .with_code("E_MACRO_MATCH")
        .related(source, definition.declaration, "macro defined here");
        for note in rejected {
            failure = failure.note(note);
        }
        Err(failure)
    }

    pub fn resume(&self, source: &str, root: &mut Sexp, request: Request) -> Result<(), Error> {
        self.resume_at(source, root, request, 0)
    }

    fn resume_at(
        &self,
        source: &str,
        root: &mut Sexp,
        request: Request,
        depth: usize,
    ) -> Result<(), Error> {
        fn replace(n: &mut Sexp, id: usize, output: &Sexp) -> bool {
            if n.context.id == id {
                *n = output.clone();
                return true;
            }
            if let Kind::List(parts) = &mut n.kind {
                for p in parts {
                    if replace(p, id, output) {
                        return true;
                    }
                }
            }
            false
        }
        if !replace(root, request.node, &request.output) {
            return Err(
                error(source, root, "macro input disappeared during elaboration")
                    .with_code("E_MACRO_CONTEXT"),
            );
        }
        let mut state = self.state.borrow_mut();
        state.report.expansions.push(request.expansion);
        drop(state);
        self.charge_expansion_at(source, root, depth)
    }

    fn charge_expansion(&self, source: &str, root: &Sexp) -> Result<(), Error> {
        self.charge_expansion_at(source, root, 0)
    }

    fn charge_expansion_at(&self, source: &str, root: &Sexp, depth: usize) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        let mut pending = vec![(root, depth)];
        while let Some((n, depth)) = pending.pop() {
            state.work += 1;
            if state.work > MAX_RECHECK_NODES || depth > 256 {
                return Err(error(source, n, "macro elaboration work limit exceeded")
                    .with_code("E_MACRO_LIMIT"));
            }
            if let Some(p) = n.list() {
                pending.extend(p.iter().map(|n| (n, depth + 1)));
            }
        }
        Ok(())
    }

    pub fn annotate(&self, source: &str, mut error: Error) -> Error {
        if let Some(span) = error.diagnostic().primary.as_ref().map(|p| p.span) {
            let report = &self.state.borrow().report;
            for e in report
                .expansions
                .iter()
                .rev()
                .filter(|e| e.call.start <= span.start && span.end <= e.call.end)
                .take(8)
            {
                error = error.related(
                    source,
                    e.rule,
                    format!("in expansion of macro `{}`", e.name),
                );
            }
        }
        error
    }
    pub fn charge_check(&self, source: &str, root: &Sexp) -> Result<(), Error> {
        if self.definitions.is_empty() {
            return Ok(());
        }
        let mut state = self.state.borrow_mut();
        let mut pending = vec![root];
        while let Some(n) = pending.pop() {
            state.work += 1;
            if state.work > MAX_RECHECK_NODES {
                return Err(error(source, n, "macro elaboration work limit exceeded")
                    .with_code("E_MACRO_LIMIT"));
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        Ok(())
    }
    fn canonical_type(&self, source: &str, node: &Sexp, depth: usize) -> Result<Sexp, Error> {
        if depth > 64 {
            return Err(
                error(source, node, "macro guard type expansion limit exceeded")
                    .with_code("E_MACRO_LIMIT"),
            );
        }
        if let Some(alias) = node.atom().and_then(|n| self.aliases.get(n)) {
            return self.canonical_type(source, alias, depth + 1);
        }
        let mut out = node.clone();
        if let Kind::List(parts) = &mut out.kind {
            for child in parts.iter_mut().skip(1) {
                *child = self.canonical_type(source, child, depth + 1)?;
            }
        }
        Ok(out)
    }
}

pub(crate) fn checked_offsets(report: &Report, offsets: &[usize]) -> Vec<usize> {
    offsets
        .iter()
        .copied()
        .filter(|offset| {
            !report
                .expansions
                .iter()
                .any(|e| e.head.start <= *offset && *offset < e.head.end)
                && !report.expansions.iter().flat_map(|e| &e.inputs).any(|i| {
                    i.outputs.is_empty() && i.source.start <= *offset && *offset < i.source.end
                })
        })
        .collect()
}
