//! Syntactic macros in otherwise arbitrary WAT. No effect/ownership guarantees
//! are inferred here: the complete output is checked by the Wasm validator.
use crate::{
    Error,
    ir::Span,
    sexpr::{Kind, Sexp},
};
use std::collections::{BTreeMap, BTreeSet};

fn render(node: &Sexp, source: &str) -> String {
    match &node.kind {
        Kind::Atom(s) => s.strip_prefix("native.").unwrap_or(s).into(),
        Kind::Text(s) => source
            .get(node.span.start..node.span.end)
            .filter(|s| s.starts_with('"'))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "\"{}\"",
                    s.as_bytes()
                        .iter()
                        .map(|b| format!("\\{b:02x}"))
                        .collect::<String>()
                )
            }),
        Kind::Bytes(bytes) => format!(
            "\"{}\"",
            bytes
                .iter()
                .map(|b| format!("\\{b:02x}"))
                .collect::<String>()
        ),
        Kind::List(items) => format!(
            "({})",
            items
                .iter()
                .map(|n| render(n, source))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

fn fields(items: &[Sexp], name: &str, source: &str) -> Vec<String> {
    items
        .iter()
        .filter(|n| n.head() == Some(name))
        .flat_map(|n| {
            let parts = n.list().unwrap();
            let start = if parts
                .get(1)
                .and_then(Sexp::atom)
                .is_some_and(|s| s.starts_with('$'))
            {
                2
            } else {
                1
            };
            parts[start..]
                .iter()
                .map(|n| render(n, source))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[derive(Clone, Default)]
struct Signature {
    params: Vec<String>,
    results: Vec<String>,
}

/// Resolve only header syntax; the Wasm validator checks type-use agreement.
struct Signatures {
    types: Vec<Option<Signature>>,
    names: BTreeMap<String, usize>,
}
impl Signatures {
    fn new(declarations: &[Sexp], source: &str) -> Self {
        let mut table = Self {
            types: vec![],
            names: BTreeMap::new(),
        };
        for node in declarations {
            let nodes = if node.head() == Some("rec") {
                &node.list().unwrap()[1..]
            } else {
                std::slice::from_ref(node)
            };
            for node in nodes.iter().filter(|n| n.head() == Some("type")) {
                let parts = node.list().unwrap();
                if let Some(name) = parts.get(1).and_then(Sexp::atom).and_then(id) {
                    table.names.insert(name, table.types.len());
                }
                let mut body = parts.last().unwrap();
                if body.head() == Some("sub") {
                    body = body.list().unwrap().last().unwrap();
                }
                table
                    .types
                    .push((body.head() == Some("func")).then(|| Signature {
                        params: fields(body.list().unwrap(), "param", source),
                        results: fields(body.list().unwrap(), "result", source),
                    }));
            }
        }
        table
    }
    fn function(&self, parts: &[Sexp], source: &str) -> Signature {
        let explicit = Signature {
            params: fields(parts, "param", source),
            results: fields(parts, "result", source),
        };
        if parts
            .iter()
            .any(|n| matches!(n.head(), Some("param" | "result")))
        {
            return explicit;
        }
        parts
            .iter()
            .find(|n| n.head() == Some("type"))
            .and_then(Sexp::list)
            .and_then(|p| p.get(1))
            .and_then(Sexp::atom)
            .and_then(|n| {
                id(n)
                    .and_then(|n| self.names.get(&n).copied())
                    .or_else(|| index(n))
            })
            .and_then(|i| self.types.get(i))
            .and_then(Option::as_ref)
            .cloned()
            .unwrap_or(explicit)
    }
}

struct LoopState {
    name: String,
    label: String,
    slots: Vec<(String, String)>,
}
struct Lower<'a> {
    source: &'a str,
    names: BTreeSet<String>,
    next: usize,
    locals: Vec<(String, String)>,
    original_locals: usize,
    declared: BTreeMap<String, String>,
    results: BTreeMap<String, Vec<String>>,
    loops: Vec<LoopState>,
}
type Scope = BTreeMap<String, (String, String)>;
fn id(name: &str) -> Option<String> {
    let mut offset = 0;
    let token = wast::lexer::Lexer::new(name).parse(&mut offset).ok()??;
    (offset == name.len() && token.kind == wast::lexer::TokenKind::Id)
        .then(|| token.id(name).ok().map(|s| format!("${s}")))
        .flatten()
}
fn index(name: &str) -> Option<usize> {
    let buffer = wast::parser::ParseBuffer::new(name).ok()?;
    wast::parser::parse::<u32>(&buffer).ok().map(|n| n as usize)
}
impl Lower<'_> {
    fn fresh(&mut self) -> String {
        loop {
            let s = format!("$__wlt_macro_{}", self.next);
            self.next += 1;
            if self.names.insert(s.clone()) {
                return s;
            }
        }
    }
    fn local(&mut self, ty: String) -> String {
        let n = self.fresh();
        self.locals.push((n.clone(), ty));
        n
    }
    fn error(&self, node: &Sexp, message: &str) -> Error {
        Error::at(self.source, node.span, message).with_code("E_WLT_NATIVE_MACRO")
    }
    fn infer(&self, node: &Sexp, scope: &Scope) -> Option<String> {
        let parts = node.list()?;
        let head = node.head()?;
        if let Some(op) = crate::ir::NumericOp::parse(head) {
            return Some(op.signature().output.wat().into());
        }
        if matches!(head, "local.get") {
            let name = parts.get(1)?.atom()?;
            let name = id(name).or_else(|| index(name).map(|n| n.to_string()))?;
            return scope
                .get(&name)
                .map(|(_, t)| t.clone())
                .or_else(|| self.declared.get(&name).cloned());
        }
        if head == "call" {
            let result = self.results.get(&id(parts.get(1)?.atom()?)?)?;
            return (result.len() == 1).then(|| result[0].clone());
        }
        if matches!(
            head,
            "struct.new"
                | "struct.new_default"
                | "array.new"
                | "array.new_default"
                | "array.new_fixed"
        ) {
            return Some(format!("(ref {})", parts.get(1)?.atom()?));
        }
        if head == "ref.null" {
            return Some(format!("(ref null {})", render(parts.get(1)?, self.source)));
        }
        if matches!(head, "block" | "if" | "native.loop") {
            let result = fields(parts, "result", self.source);
            return (result.len() == 1).then(|| result[0].clone());
        }
        if matches!(
            head,
            "memory.size"
                | "memory.grow"
                | "table.size"
                | "table.grow"
                | "ref.is_null"
                | "ref.eq"
                | "array.len"
        ) {
            return Some("i32".into());
        }
        for ty in ["i32", "i64", "f32", "f64", "v128"] {
            if head.starts_with(&format!("{ty}.")) && !head.contains("store") {
                return Some(
                    if matches!(
                        head.rsplit('.').next()?,
                        "eq" | "ne"
                            | "eqz"
                            | "lt_s"
                            | "lt_u"
                            | "gt_s"
                            | "gt_u"
                            | "le_s"
                            | "le_u"
                            | "ge_s"
                            | "ge_u"
                            | "lt"
                            | "gt"
                            | "le"
                            | "ge"
                    ) {
                        "i32"
                    } else {
                        ty
                    }
                    .into(),
                );
            }
        }
        None
    }
    fn sequence(
        &mut self,
        nodes: &[Sexp],
        scope: &Scope,
        tail: bool,
    ) -> Result<Vec<String>, Error> {
        let mut out = vec![];
        let mut i = 0;
        while i < nodes.len() {
            if let Some(op @ ("local.get" | "local.set" | "local.tee")) = nodes[i].atom()
                && let Some(operand) = nodes.get(i + 1)
            {
                out.push(op.into());
                out.push(self.operand(op, operand, scope)?);
                i += 2;
                continue;
            }
            out.extend(self.expr(&nodes[i], scope, tail && i + 1 == nodes.len())?);
            i += 1;
        }
        Ok(out)
    }
    fn operand(&self, op: &str, node: &Sexp, scope: &Scope) -> Result<String, Error> {
        let name = node.atom().unwrap_or("");
        if let Some((actual, _)) = id(name).as_ref().and_then(|n| scope.get(n)) {
            if op != "local.get" {
                return Err(self.error(
                    node,
                    "%let and loop state bindings are immutable; use %recur for state updates",
                ));
            }
            return Ok(actual.clone());
        }
        if index(name).is_some_and(|i| i >= self.original_locals) {
            return Err(self.error(
                node,
                "numeric locals refer only to original WAT parameters and locals",
            ));
        }
        Ok(render(node, self.source))
    }
    fn expr(&mut self, node: &Sexp, scope: &Scope, tail: bool) -> Result<Vec<String>, Error> {
        let Some(parts) = node.list() else {
            return Ok(vec![render(node, self.source)]);
        };
        let head = node.head().unwrap_or("");
        if head.starts_with('@') {
            return Ok(vec![render(node, self.source)]);
        }
        if head == "let" {
            if !(4..=5).contains(&parts.len()) || parts[1].atom().and_then(id).is_none() {
                return Err(self.error(
                    node,
                    "native %let requires (%let $name [type] initial body)",
                ));
            }
            let index = if parts.len() == 5 { 3 } else { 2 };
            let ty = if index == 3 {
                render(&parts[2], self.source)
            } else {
                self.infer(&parts[index], scope).ok_or_else(|| {
                    self.error(
                        &parts[index],
                        "cannot infer native initializer type; add an explicit %let type",
                    )
                })?
            };
            let name = self.local(ty.clone());
            let initial = self.expr(&parts[index], scope, false)?.join(" ");
            let mut nested = scope.clone();
            nested.insert(id(parts[1].atom().unwrap()).unwrap(), (name.clone(), ty));
            let mut out = vec![format!("(local.set {name} {initial})")];
            out.extend(self.expr(&parts[index + 1], &nested, tail)?);
            return Ok(out);
        }
        if matches!(head, "loop" | "for") {
            return self.state_loop(node, scope);
        }
        if head == "recur" {
            if !tail {
                return Err(self.error(node, "%recur must be in tail position"));
            }
            let name = parts
                .get(1)
                .and_then(Sexp::atom)
                .and_then(id)
                .unwrap_or_default();
            let state = self
                .loops
                .last()
                .filter(|l| l.name == name)
                .ok_or_else(|| self.error(node, "%recur must target the innermost loop"))?;
            let label = state.label.clone();
            let slots = state.slots.clone();
            if parts.len() != slots.len() + 2 {
                return Err(self.error(node, "%recur argument count mismatch"));
            }
            let mut out = vec![];
            let mut temporaries = vec![];
            for ((_, ty), arg) in slots.iter().zip(&parts[2..]) {
                let temp = self.local(ty.clone());
                out.push(format!(
                    "(local.set {temp} {})",
                    self.expr(arg, scope, false)?.join(" ")
                ));
                temporaries.push(temp);
            }
            for ((slot, _), temp) in slots.iter().zip(temporaries) {
                out.push(format!("(local.set {slot} (local.get {temp}))"));
            }
            out.push(format!("(br {label})"));
            return Ok(out);
        }
        let mut out = vec![head.strip_prefix("native.").unwrap_or(head).to_owned()];
        if matches!(head, "local.get" | "local.set" | "local.tee") && parts.len() >= 2 {
            out.push(self.operand(head, &parts[1], scope)?);
            out.extend(self.sequence(&parts[2..], scope, false)?);
        } else {
            for (i, child) in parts.iter().enumerate().skip(1) {
                let child_tail = tail
                    && (matches!(head, "block" | "then" | "else") && i + 1 == parts.len()
                        || head == "if" && matches!(child.head(), Some("then" | "else")));
                out.extend(self.expr(child, scope, child_tail)?);
            }
        }
        Ok(vec![format!("({})", out.join(" "))])
    }
    fn state_loop(&mut self, node: &Sexp, scope: &Scope) -> Result<Vec<String>, Error> {
        let parts = node.list().unwrap();
        let name = parts
            .get(1)
            .and_then(Sexp::atom)
            .and_then(id)
            .ok_or_else(|| self.error(node, "missing loop label"))?;
        let label = self.fresh();
        let mut nested = scope.clone();
        let mut slots = vec![];
        let mut out = vec![];
        let mut i = 2;
        let mut bound = BTreeSet::new();
        while let Some(binding) = parts.get(i).filter(|n| n.head() == Some("bind")) {
            let b = binding.list().unwrap();
            if b.len() != 4 || b[1].atom().and_then(id).is_none() {
                return Err(self.error(binding, "expected (bind $name type initial)"));
            }
            let n = id(b[1].atom().unwrap()).unwrap();
            if !bound.insert(n.clone()) {
                return Err(self.error(binding, "duplicate loop binding"));
            }
            let ty = render(&b[2], self.source);
            let slot = self.local(ty.clone());
            out.push(format!(
                "(local.set {slot} {})",
                self.expr(&b[3], scope, false)?.join(" ")
            ));
            nested.insert(n, (slot.clone(), ty.clone()));
            slots.push((slot, ty));
            i += 1;
        }
        self.loops.push(LoopState {
            name,
            label: label.clone(),
            slots,
        });
        let body = if node.head() == Some("for") {
            if i == 2
                || parts.len() != i + 3
                || parts[i].head() != Some("while")
                || parts[i + 1].head() != Some("step")
                || parts[i].list().unwrap().len() != 2
            {
                return Err(self.error(node, "expected %for bindings, while, step and unit body"));
            }
            let condition = self
                .expr(&parts[i].list().unwrap()[1], &nested, false)?
                .join(" ");
            let user_body = self.expr(&parts[i + 2], &nested, false)?.join(" ");
            let mut args = parts[i + 1].list().unwrap().to_vec();
            args[0].kind = Kind::Atom("recur".into());
            args.insert(1, parts[1].clone());
            let next = self
                .expr(
                    &Sexp {
                        kind: Kind::List(args),
                        span: node.span,
                    },
                    &nested,
                    true,
                )?
                .join(" ");
            format!("(if {condition} (then {user_body} {next}))")
        } else {
            let result = if parts.get(i).and_then(Sexp::head) == Some("result") {
                let r = render(&parts[i], self.source);
                i += 1;
                r
            } else {
                String::new()
            };
            if parts.len() != i + 1 {
                return Err(self.error(node, "expected one %loop body"));
            }
            format!(
                "{result} {}",
                self.expr(&parts[i], &nested, true)?.join(" ")
            )
        };
        self.loops.pop();
        out.push(format!("(loop {label} {body})"));
        Ok(out)
    }
}

pub(crate) fn expand(source: &str) -> Result<Option<String>, Error> {
    let root = crate::sexpr::parse(source)?;
    let standard = crate::standard::prepare(source, &root)?;
    let raw = standard.syntax(root)?;
    let source = standard.source(source);
    let mut pending = vec![&raw];
    let mut found = false;
    let mut names = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if node.head().is_some_and(|h| h.starts_with('@')) {
            continue;
        }
        if node.head() == Some("opaque") {
            return Ok(None);
        }
        if let Some(name) = node.atom() {
            names.insert(name.into());
            if let Some(name) = id(name) {
                names.insert(name);
            }
            if let Some(name) = name.strip_prefix('%') {
                found = true;
                if !matches!(
                    name,
                    "let"
                        | "loop"
                        | "for"
                        | "recur"
                        | "include"
                        | "import"
                        | "export"
                        | "namespace"
                        | "public"
                        | "use"
                        | "file.namespace"
                        | "type.alias"
                        | "type.params"
                        | "inst"
                ) {
                    return Ok(None);
                }
            }
        }
        if let Some(items) = node.list() {
            pending.extend(items);
        }
    }
    if !found {
        return Ok(None);
    }
    let mut normalized = crate::syntax::normalize(source, &raw)?;
    standard.rewrite(&mut normalized);
    let modules = crate::modules::expand(source, &normalized)?;
    let generic = crate::generics::expand(source, &modules.syntax)?;
    let aliases = crate::aliases::expand(source, &generic.syntax)?;
    let mut root = aliases.syntax;
    let Some(declarations) = root.list().filter(|_| root.head() == Some("module")) else {
        return Ok(None);
    };
    let signatures = Signatures::new(&declarations[1..], source);
    let mut results = BTreeMap::new();
    for node in &declarations[1..] {
        let function = if node.head() == Some("import") {
            node.list().and_then(|p| p.get(3))
        } else if node.head() == Some("func") {
            Some(node)
        } else {
            None
        };
        if let Some(function) = function
            && let Some(name) = function.list().and_then(|p| p.get(1)).and_then(Sexp::atom)
        {
            results.insert(
                id(name).unwrap_or_else(|| name.into()),
                signatures
                    .function(function.list().unwrap(), source)
                    .results,
            );
        }
    }
    let Kind::List(declarations) = &mut root.kind else {
        unreachable!()
    };
    let mut output = vec!["module".into()];
    let mut lower = Lower {
        source,
        names,
        next: 0,
        locals: vec![],
        original_locals: 0,
        declared: BTreeMap::new(),
        results,
        loops: vec![],
    };
    for node in &declarations[1..] {
        if node.head() != Some("func") {
            output.push(render(node, source));
            continue;
        }
        let parts = node.list().unwrap();
        let start = parts
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, n)| {
                n.atom().is_none_or(|n| !n.starts_with('$'))
                    && !matches!(
                        n.head(),
                        Some("param" | "result" | "local" | "type" | "export" | "import")
                    )
                    && !n.head().is_some_and(|h| h.starts_with('@'))
            })
            .map(|(i, _)| i)
            .unwrap_or(parts.len());
        lower.locals.clear();
        lower.declared.clear();
        let params = signatures.function(parts, source).params;
        let locals = fields(parts, "local", source);
        lower.original_locals = params.len() + locals.len();
        for (i, ty) in params.iter().enumerate() {
            lower.declared.insert(i.to_string(), ty.clone());
        }
        let mut index = if parts.iter().any(|n| n.head() == Some("param")) {
            0
        } else {
            params.len()
        };
        for declaration in parts
            .iter()
            .filter(|n| matches!(n.head(), Some("param" | "local")))
        {
            let p = declaration.list().unwrap();
            let named = p.get(1).and_then(Sexp::atom).filter(|n| n.starts_with('$'));
            for ty in p.iter().skip(if named.is_some() { 2 } else { 1 }) {
                let ty = render(ty, source);
                lower.declared.insert(index.to_string(), ty.clone());
                if let Some(name) = named.and_then(id) {
                    lower.declared.insert(name, ty);
                }
                index += 1;
            }
        }
        let body = lower
            .sequence(&parts[start..], &Scope::new(), true)?
            .join(" ");
        let locals = lower
            .locals
            .iter()
            .map(|(n, t)| format!("(local {n} {t})"))
            .collect::<Vec<_>>()
            .join(" ");
        output.push(format!(
            "({} {locals} {body})",
            parts[..start]
                .iter()
                .map(|n| render(n, source))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    let wat = format!("({})", output.join("\n"));
    // Unused aliases are obligations too; temporary probes never enter output.
    if !aliases.targets.is_empty() {
        let probes = aliases
            .targets
            .iter()
            .map(|t| format!("(func (result {}) (unreachable))", render(t, source)))
            .collect::<String>();
        let probe = format!("{} {probes})", &wat[..wat.len() - 1]);
        let bytes = wat::parse_str(probe)
            .map_err(|e| Error::at(source, Span::default(), e).with_code("E_NATIVE_WAT"))?;
        wasmparser::Validator::new()
            .validate_all(&bytes)
            .map_err(|e| Error::at(source, Span::default(), e).with_code("E_NATIVE_WAT"))?;
    }
    Ok(Some(wat))
}
