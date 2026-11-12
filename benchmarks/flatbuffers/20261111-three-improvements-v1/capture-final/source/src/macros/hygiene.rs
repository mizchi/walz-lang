//! Lexical alpha-renaming before substitution. Captured syntax keeps its own
//! context; intentional binding is possible only through a checked bind clause.
use super::*;
use crate::sexpr::SyntaxContext;

type Scope = BTreeMap<String, String>;

fn slot(name: &str, index: Option<usize>, rule: &Rule) -> String {
    if rule.variables.get(name).is_some_and(|v| v.repeated) {
        format!("{name}@{}", index.expect("repeated binder index"))
    } else {
        name.into()
    }
}

struct Instantiator<'a> {
    source: &'a str,
    definitions: &'a BTreeMap<String, Macro>,
    delegated: BTreeSet<super::bindings::Delegated>,
    call: &'a Sexp,
    rule: &'a Rule,
    captures: &'a Captures,
    id: usize,
    names: &'a mut BTreeSet<String>,
    next: &'a mut usize,
    bound: BTreeSet<(String, String)>,
    created: BTreeSet<String>,
    nodes: usize,
    bytes: usize,
}

pub(super) fn instantiate(
    source: &str,
    call: &Sexp,
    rule: &Rule,
    captures: &Captures,
    id: usize,
    fresh: (&mut BTreeSet<String>, &mut usize),
    definitions: &BTreeMap<String, Macro>,
) -> Result<Sexp, Error> {
    let (names, next) = fresh;
    let mut expander = Instantiator {
        source,
        definitions,
        delegated: BTreeSet::new(),
        call,
        rule,
        captures,
        id,
        names,
        next,
        bound: BTreeSet::new(),
        created: BTreeSet::new(),
        nodes: 0,
        bytes: 0,
    };
    let output = expander.walk(&rule.template, &Scope::new(), false, None, 0)?;
    if rule
        .bindings
        .iter()
        .any(|b| !expander.bound.contains(b) && !captures[&b.0].values().is_empty())
    {
        return Err(error(
            source,
            call,
            "bind clause does not match a binding and body in the template",
        )
        .with_code("E_MACRO_BINDING"));
    }
    Ok(output)
}

impl Instantiator<'_> {
    fn charge(&mut self, n: &Sexp, depth: usize) -> Result<(), Error> {
        self.nodes += 1;
        self.bytes += match &n.kind {
            Kind::Atom(s) | Kind::Text(s) => s.len().saturating_mul(6),
            Kind::Bytes(b) => b.len().saturating_mul(3),
            Kind::List(_) => 3,
        };
        if depth > 256 || self.nodes > MAX_NODES || self.bytes > 4 * 1024 * 1024 {
            return Err(error(
                self.source,
                self.call,
                "macro template expansion size/nesting limit exceeded",
            )
            .with_code("E_MACRO_LIMIT"));
        }
        Ok(())
    }
    fn fresh(&mut self) -> String {
        loop {
            let name = format!("$macro_scope_{}", self.next);
            *self.next += 1;
            if self.names.insert(name.clone()) {
                return name;
            }
        }
    }
    fn stamp(&self, n: &Sexp, input: bool) -> Sexp {
        let mut n = n.clone();
        if !input {
            n.context = SyntaxContext {
                id: 0,
                expansion: Some(self.id),
                template: Some(n.span),
                global: n.context.global,
            };
            n.span = if matches!(n.kind, Kind::List(_)) {
                self.call.span
            } else {
                self.call
                    .list()
                    .and_then(|p| p.first())
                    .map(|n| n.span)
                    .unwrap_or(self.call.span)
            };
        } else {
            n.context.expansion = Some(self.id);
        }
        n
    }
    fn data(
        &mut self,
        n: &Sexp,
        input: bool,
        index: Option<usize>,
        depth: usize,
    ) -> Result<Sexp, Error> {
        self.charge(n, depth)?;
        if !input && n.atom().is_some_and(|a| a.starts_with('?')) {
            return self.capture(n, &Scope::new(), index, depth);
        }
        let mut out = self.stamp(n, input);
        if let Kind::List(parts) = &mut out.kind {
            for child in parts {
                *child = self.data(child, input, index, depth + 1)?;
            }
        }
        Ok(out)
    }
    fn capture(
        &mut self,
        n: &Sexp,
        scope: &Scope,
        index: Option<usize>,
        depth: usize,
    ) -> Result<Sexp, Error> {
        let name = n.atom().unwrap();
        let value = match self.captures.get(name) {
            Some(Capture::One(v)) => v,
            Some(Capture::Many(v)) => index.and_then(|i| v.get(i)).ok_or_else(|| {
                error(
                    self.source,
                    n,
                    "repeated variable must occur in a repeated template",
                )
                .help("place the repeated term followed by `...` at the end of its list; bind a final body separately or use recursive macro rules")
            })?,
            None => return Err(error(self.source, n, "unknown template variable")),
        };
        let mut nested = Scope::new();
        for (binder, body) in &self.rule.bindings {
            if body == name {
                for (i, original) in self.captures[binder].values().iter().enumerate() {
                    let binder_index = self.rule.variables[binder].repeated.then_some(i);
                    let key = slot(binder, binder_index, self.rule);
                    if scope.get(&key).is_none()
                        && self.delegated.contains(&super::bindings::Delegated {
                            binder: binder.clone(),
                            body: body.clone(),
                            occurrence: n.span,
                            index,
                            binder_index,
                        })
                    {
                        self.bound.insert((binder.clone(), body.clone()));
                        continue;
                    }
                    let fresh = scope.get(&key).ok_or_else(|| {
                        error(
                            self.source,
                            n,
                            "declared body is outside its binder's lexical scope",
                        )
                        .with_code("E_MACRO_BINDING")
                        .help("forward the captured body unchanged as the callee's entire body argument; put any wrapper inside the callee's binding scope instead")
                    })?;
                    nested.insert(original.atom().unwrap().into(), fresh.clone());
                    self.bound.insert((binder.clone(), body.clone()));
                }
            }
        }
        let key = if self.rule.variables.get(name).is_some_and(|v| v.repeated) {
            slot(name, index, self.rule)
        } else {
            name.into()
        };
        if let Some(fresh) = scope.get(&key) {
            let mut out = value.clone();
            out.kind = Kind::Atom(fresh.clone());
            return Ok(out);
        }
        // Types and identifiers are syntax, not implicitly evaluated locals.
        if self
            .rule
            .variables
            .get(name)
            .is_none_or(|v| v.category != Category::Expr)
        {
            return self.data(value, true, index, depth + 1);
        }
        self.walk(value, &nested, true, None, depth + 1)
    }
    fn binder(
        &mut self,
        n: &Sexp,
        scope: &mut Scope,
        input: bool,
        index: Option<usize>,
        depth: usize,
    ) -> Result<Sexp, Error> {
        self.charge(n, depth)?;
        if let Some(name) = n.atom() {
            let mut out = self.stamp(n, input);
            if !input && name.starts_with('?') {
                if !self.created.insert(slot(name, index, self.rule)) {
                    return Err(error(
                        self.source,
                        n,
                        "a caller-supplied binder must be introduced once",
                    )
                    .with_code("E_MACRO_BINDING"));
                }
                if !self.rule.bindings.iter().any(|(b, _)| b == name) {
                    return Err(error(
                        self.source,
                        n,
                        "caller-supplied binder requires a bind clause",
                    )
                    .with_code("E_MACRO_BINDING"));
                }
                let capture = self
                    .captures
                    .get(name)
                    .ok_or_else(|| error(self.source, n, "missing binder fragment"))?;
                let value = match capture {
                    Capture::One(n) => Some(n),
                    Capture::Many(ns) => index.and_then(|i| ns.get(i)),
                }
                .ok_or_else(|| {
                    error(
                        self.source,
                        n,
                        "repeated binder requires a template repetition",
                    )
                    .with_code("E_MACRO_BINDING")
                })?;
                out = value.clone();
            } else if !name.starts_with('$') {
                return Err(error(self.source, n, "macro binder requires an identifier")
                    .with_code("E_MACRO_BINDING"));
            }
            let fresh = if input { name.into() } else { self.fresh() };
            if !input && !name.starts_with('?') {
                out.span.end = out.span.start;
            }
            let key = if !input && name.starts_with('?') {
                slot(name, index, self.rule)
            } else {
                name.into()
            };
            scope.insert(key, fresh.clone());
            out.kind = Kind::Atom(fresh);
            return Ok(out);
        }
        let mut out = self.stamp(n, input);
        if let Kind::List(parts) = &mut out.kind {
            if n.head() == Some("@fields") {
                for field in parts.iter_mut().skip(1) {
                    if let Kind::List(p) = &mut field.kind
                        && p.len() == 2
                    {
                        p[1] = self.binder(&p[1], scope, input, index, depth + 1)?;
                    }
                }
            } else {
                for child in parts {
                    *child = self.binder(child, scope, input, index, depth + 1)?;
                }
            }
        }
        Ok(out)
    }
    fn repeat_len(&self, n: &Sexp) -> Result<usize, Error> {
        let mut pending = vec![n];
        let mut length = None;
        while let Some(n) = pending.pop() {
            if let Some(Capture::Many(v)) = n.atom().and_then(|s| self.captures.get(s)) {
                if length.is_some_and(|n| n != v.len()) {
                    return Err(error(
                        self.source,
                        n,
                        "repeated captures have different lengths",
                    ));
                }
                length = Some(v.len());
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        length.ok_or_else(|| {
            error(
                self.source,
                n,
                "template repetition requires a repeated pattern variable",
            )
        })
    }
    fn walk(
        &mut self,
        n: &Sexp,
        scope: &Scope,
        input: bool,
        index: Option<usize>,
        depth: usize,
    ) -> Result<Sexp, Error> {
        self.charge(n, depth)?;
        if let Some(name) = n.atom() {
            if !input && name.starts_with('?') {
                return self.capture(n, scope, index, depth);
            }
            let mut out = self.stamp(n, input);
            if name.starts_with('$') {
                if let Some(fresh) = scope.get(name) {
                    out.kind = Kind::Atom(fresh.clone());
                } else if !input {
                    return Err(error(
                        self.source,
                        n,
                        format!("free template local `{name}`; pass it as an expr fragment"),
                    )
                    .with_code("E_MACRO_HYGIENE"));
                }
            }
            return Ok(out);
        }
        let Some(p) = n.list() else {
            return Ok(self.stamp(n, input));
        };
        if !input
            && let Some(marker) = p
                .iter()
                .take(p.len().saturating_sub(1))
                .find(|n| n.atom() == Some("..."))
        {
            return Err(error(self.source, marker, "template repetition must end its list")
                .help("place the repeated term followed by `...` at the end of its list; use recursive macro rules when a final body must follow repeated bindings"));
        }
        let head = n.head().unwrap_or("");
        let mut out = self.stamp(n, input);
        let mut fields = vec![];
        let start = usize::from(p.first().is_some_and(|n| n.atom().is_some()));
        if start == 1 {
            let first = &p[0];
            fields.push(
                if !input && first.atom().is_some_and(|n| n.starts_with('?')) {
                    self.capture(first, scope, index, depth + 1)?
                } else {
                    self.stamp(first, input)
                },
            );
        }
        // A single trailing repetition, with the same rule as the matcher.
        let repeated = !input && p.last().and_then(Sexp::atom) == Some("...");
        if repeated {
            if p.len() < start + 2 || index.is_some() {
                return Err(error(
                    self.source,
                    n,
                    "nested template repetition is not supported",
                ));
            }
            for child in &p[start..p.len() - 2] {
                fields.push(self.walk(child, scope, input, index, depth + 1)?);
            }
            for i in 0..self.repeat_len(&p[p.len() - 2])? {
                fields.push(self.walk(&p[p.len() - 2], scope, false, Some(i), depth + 1)?);
            }
            out.kind = Kind::List(fields);
            return Ok(out);
        }
        if head == "let" && p.len() == 3 && p[1].list().is_some() {
            let group = p[1].list().unwrap();
            let mut entries = vec![];
            if !input && group.last().and_then(Sexp::atom) == Some("...") {
                if group.len() < 2 || index.is_some() {
                    return Err(error(
                        self.source,
                        &p[1],
                        "nested template repetition is not supported",
                    ));
                }
                entries.extend(group[..group.len() - 2].iter().map(|n| (n, index)));
                for i in 0..self.repeat_len(&group[group.len() - 2])? {
                    entries.push((&group[group.len() - 2], Some(i)));
                }
            } else {
                entries.extend(group.iter().map(|n| (n, index)));
            }
            if entries.len() > 256 {
                return Err(error(self.source, n, "macro binding group limit exceeded")
                    .with_code("E_MACRO_LIMIT"));
            }
            let mut nested = scope.clone();
            let mut bindings = vec![];
            for (entry, index) in entries {
                let b = entry
                    .list()
                    .filter(|p| (2..=3).contains(&p.len()))
                    .ok_or_else(|| {
                        error(
                            self.source,
                            entry,
                            "expected ($name [type] initializer) in macro binding group",
                        )
                        .with_code("E_MACRO_BINDING")
                    })?;
                let initial = self.walk(b.last().unwrap(), &nested, input, index, depth + 1)?;
                let ty = if b.len() == 3 {
                    let annotation = if b[1].head() == Some("type") {
                        b[1].list().and_then(|p| p.get(1)).unwrap_or(&b[1])
                    } else {
                        &b[1]
                    };
                    Some(self.data(annotation, input, index, depth + 1)?)
                } else {
                    None
                };
                let binder = self.binder(&b[0], &mut nested, input, index, depth + 1)?;
                bindings.push((binder, ty, initial));
            }
            let mut body = self.walk(&p[2], &nested, input, index, depth + 1)?;
            for (binder, ty, initial) in bindings.into_iter().rev() {
                let mut binding = self.stamp(n, input);
                let mut parts = vec![self.stamp(&p[0], input), binder];
                parts.extend(ty);
                parts.push(initial);
                parts.push(body);
                binding.kind = Kind::List(parts);
                body = binding;
            }
            return Ok(body);
        }
        if matches!(head, "let" | "using" | "for-each" | "result.let") && p.len() >= 4 {
            let mut nested = scope.clone();
            let binder = self.binder(&p[1], &mut nested, input, index, depth + 1)?;
            fields.push(binder);
            let last = p.len() - 1;
            for (i, child) in p.iter().enumerate().skip(2) {
                let is_type = matches!(head, "let" | "using") && p.len() == 5 && i == 2
                    || head == "result.let" && p.len() == 6 && i < 4;
                fields.push(if is_type {
                    self.data(child, input, index, depth + 1)?
                } else {
                    self.walk(
                        child,
                        if i == last { &nested } else { scope },
                        input,
                        index,
                        depth + 1,
                    )?
                });
            }
        } else if head == "result.do" && p.len() >= 3 {
            let mut nested = scope.clone();
            for binding in &p[1..p.len() - 1] {
                let b = binding.list().unwrap_or(&[]);
                if b.len() != 2 {
                    return Err(error(self.source, binding, "expected result.do binding"));
                }
                let initial = self.walk(&b[1], &nested, input, index, depth + 1)?;
                let name = if b[0].list().is_some_and(|p| p.is_empty()) {
                    self.stamp(&b[0], input)
                } else {
                    self.binder(&b[0], &mut nested, input, index, depth + 1)?
                };
                let mut next = self.stamp(binding, input);
                next.kind = Kind::List(vec![name, initial]);
                fields.push(next);
            }
            fields.push(self.walk(p.last().unwrap(), &nested, input, index, depth + 1)?);
        } else if head == "stream.fold-result" && matches!(p.len(), 5 | 8) {
            let (start, binders) = if p.len() == 8 { (4, 6) } else { (1, 3) };
            for ty in &p[1..start] {
                fields.push(self.data(ty, input, index, depth + 1)?);
            }
            for initial in &p[start..binders] {
                fields.push(self.walk(initial, scope, input, index, depth + 1)?);
            }
            let mut nested = scope.clone();
            fields.push(self.binder(&p[binders], &mut nested, input, index, depth + 1)?);
            fields.push(self.walk(&p[binders + 1], &nested, input, index, depth + 1)?);
        } else if matches!(head, "loop" | "for") && p.len() >= 3 {
            let mut nested = scope.clone();
            // Loop labels inhabit a separate namespace from local variables.
            let label_key = format!("@label:{}", p[1].atom().unwrap_or(""));
            let mut label = self.data(&p[1], input, index, depth + 1)?;
            if !input {
                let fresh = self.fresh();
                nested.insert(label_key, fresh.clone());
                label.kind = Kind::Atom(fresh);
            }
            fields.push(label);
            for child in p.iter().skip(2).take_while(|n| n.head() == Some("bind")) {
                let b = child.list().unwrap();
                if b.len() != 4 {
                    return Err(error(self.source, child, "expected loop binding"));
                }
                let mut binding = self.stamp(child, input);
                binding.kind = Kind::List(vec![
                    self.stamp(&b[0], input),
                    self.binder(&b[1], &mut nested, input, index, depth + 1)?,
                    self.data(&b[2], input, index, depth + 1)?,
                    self.walk(&b[3], scope, input, index, depth + 1)?,
                ]);
                fields.push(binding);
            }
            for child in p.iter().skip(fields.len()) {
                fields.push(self.walk(child, &nested, input, index, depth + 1)?);
            }
        } else if matches!(head, "case" | "catch" | "catch_all") {
            let binder_index = if head == "catch_all" { 1 } else { 2 };
            let mut nested = scope.clone();
            for (i, child) in p.iter().enumerate().skip(1) {
                fields.push(if i == binder_index {
                    self.binder(child, &mut nested, input, index, depth + 1)?
                } else if i < binder_index {
                    self.data(child, input, index, depth + 1)?
                } else {
                    self.walk(child, &nested, input, index, depth + 1)?
                });
            }
        } else if head == "struct.update" && p.len() == 4 {
            fields.push(self.data(&p[1], input, index, depth + 1)?);
            fields.push(self.walk(&p[2], scope, input, index, depth + 1)?);
            let b = p[3].list().unwrap_or(&[]);
            if b.len() != 3 {
                return Err(error(self.source, &p[3], "expected field update binding"));
            }
            let mut nested = scope.clone();
            let mut binding = self.stamp(&p[3], input);
            binding.kind = Kind::List(vec![
                self.data(&b[0], input, index, depth + 1)?,
                self.binder(&b[1], &mut nested, input, index, depth + 1)?,
                self.walk(&b[2], &nested, input, index, depth + 1)?,
            ]);
            fields.push(binding);
        } else if !input && head.starts_with("@$") {
            let prior = self.delegated.clone();
            self.delegated.extend(super::bindings::delegation(
                self.source,
                n,
                self.captures,
                index,
                self.rule,
                self.definitions,
                MAX_NODES - self.nodes,
            )?);
            for child in &p[1..] {
                fields.push(self.walk(child, scope, input, index, depth + 1)?);
            }
            self.delegated = prior;
        } else {
            for (i, child) in p.iter().enumerate().skip(start) {
                let global = child.context.global
                    || matches!(
                        head,
                        "call"
                            | "closure.call"
                            | "closure.new"
                            | "ref.null"
                            | "struct.new"
                            | "struct.named"
                            | "struct.get"
                            | "struct.set"
                            | "array.new"
                            | "array.new_fixed"
                            | "array.get"
                            | "array.get_s"
                            | "array.get_u"
                            | "array.set"
                            | "array.copy"
                            | "bytes.from-array"
                            | "bytes.copy-to-array"
                            | "variant.new"
                            | "newtype.wrap"
                            | "newtype.unwrap"
                            | "resource.new"
                            | "throw"
                    ) && i == 1
                    || head == "array.copy" && i == 2;
                let field = matches!(head, "struct.get" | "struct.set" | "variant.new") && i == 2;
                let ty = matches!(head, "ref" | "result" | "type" | "param" | "effect")
                    || head == "match"
                        && i == 1
                        && !matches!(p.get(2).and_then(Sexp::head), Some("case" | "default"))
                    || head.ends_with(".const");
                if matches!(head, "recur" | "br" | "br_if") && i == 1 {
                    let mut label = self.data(child, input, index, depth + 1)?;
                    if let Some(value) =
                        child.atom().and_then(|n| scope.get(&format!("@label:{n}")))
                    {
                        label.kind = Kind::Atom(value.clone());
                    }
                    fields.push(label);
                } else if global || field || ty {
                    fields.push(self.data(child, input, index, depth + 1)?);
                } else {
                    fields.push(self.walk(child, scope, input, index, depth + 1)?);
                }
            }
        }
        out.kind = Kind::List(fields);
        Ok(out)
    }
}
