//! Binding interfaces are syntax-only contracts. Delegation is permitted only
//! when every syntactically matching rule advertises the same input scopes.
use super::*;

impl Environment {
    /// Explain a known caller binder used in a captured expression outside the
    /// declared body. Match source identities, never a rendered error message.
    pub fn caller_scope_binding(&self, source: &str, node: &Sexp, name: &str) -> Option<Span> {
        let state = self.state.borrow();
        for expansion in state.report.expansions.iter().rev() {
            let rule = self
                .definitions
                .get(&expansion.name)?
                .rules
                .iter()
                .find(|r| r.span == expansion.rule)?;
            let outside_body = expansion.inputs.iter().any(|input| {
                input.source.start <= node.span.start
                    && node.span.end <= input.source.end
                    && rule
                        .variables
                        .get(&input.name)
                        .is_some_and(|v| v.category == Category::Expr)
                    && !rule.bindings.iter().any(|(_, body)| body == &input.name)
            });
            if outside_body
                && rule.bindings.iter().any(|(binder, _)| {
                    expansion.inputs.iter().any(|input| {
                        &input.name == binder
                            && source.get(input.source.start..input.source.end) == Some(name)
                    })
                })
            {
                return Some(rule.span);
            }
        }
        None
    }
}

impl Macro {
    pub(super) fn binding_inputs(&self, node: &Sexp) -> Option<Vec<(Sexp, Sexp)>> {
        let mut args = node.clone();
        if let Kind::List(p) = &mut args.kind {
            p.remove(0);
        }
        let mut answer: Option<Vec<(Sexp, Sexp)>> = None;
        for rule in &self.rules {
            let mut captures = Captures::new();
            if !matches(&rule.pattern, &args, false, &mut captures) {
                continue;
            }
            for (name, v) in &rule.variables {
                if v.repeated {
                    captures
                        .entry(name.clone())
                        .or_insert_with(|| Capture::Many(vec![]));
                }
            }
            let mut pairs = vec![];
            for (binder, body) in &rule.bindings {
                let body = captures.get(body)?.one()?;
                for name in captures.get(binder)?.values() {
                    pairs.push((name.clone(), body.clone()));
                }
            }
            if let Some(previous) = &answer {
                if previous
                    .iter()
                    .map(|(a, b)| (a.context.id, b.context.id, a.span, b.span))
                    .ne(pairs
                        .iter()
                        .map(|(a, b)| (a.context.id, b.context.id, a.span, b.span)))
                {
                    return None;
                }
            } else {
                answer = Some(pairs);
            }
        }
        answer
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Delegated {
    pub binder: String,
    pub body: String,
    pub occurrence: Span,
    pub index: Option<usize>,
    pub binder_index: Option<usize>,
}

pub(super) fn delegation(
    source: &str,
    node: &Sexp,
    captures: &Captures,
    index: Option<usize>,
    rule: &Rule,
    definitions: &BTreeMap<String, Macro>,
    budget: usize,
) -> Result<BTreeSet<Delegated>, Error> {
    fn preview(
        n: &Sexp,
        captures: &Captures,
        index: Option<usize>,
        next: &mut usize,
        variables: &mut BTreeMap<usize, (String, Span, Option<usize>)>,
        work: &mut usize,
    ) -> Result<Sexp, ()> {
        *work = work.checked_sub(1).ok_or(())?;
        let id = *next;
        *next += 1;
        if let Some((name, capture)) = n
            .atom()
            .and_then(|name| captures.get(name).map(|c| (name, c)))
        {
            let value = match capture {
                Capture::One(n) => Some(n),
                Capture::Many(ns) => index.and_then(|i| ns.get(i)),
            };
            if let Some(value) = value {
                let mut pending = vec![value];
                while let Some(n) = pending.pop() {
                    *work = work.checked_sub(1).ok_or(())?;
                    if let Some(p) = n.list() {
                        pending.extend(p);
                    }
                }
                let mut out = value.clone();
                fn identify(n: &mut Sexp, next: &mut usize) {
                    n.context.id = *next;
                    *next += 1;
                    if let Kind::List(p) = &mut n.kind {
                        for c in p {
                            identify(c, next);
                        }
                    }
                }
                identify(&mut out, next);
                variables.insert(out.context.id, (name.into(), n.span, index));
                return Ok(out);
            }
        }
        let mut out = Sexp {
            kind: if n.list().is_some() {
                Kind::List(vec![])
            } else {
                n.kind.clone()
            },
            span: n.span,
            context: n.context,
        };
        out.context.id = id;
        if let Kind::List(p) = &n.kind {
            let repeated =
                p.last().and_then(Sexp::atom) == Some("...") && p.len() >= 2 && index.is_none();
            let mut parts = vec![];
            let end = if repeated { p.len() - 2 } else { p.len() };
            for c in &p[..end] {
                parts.push(preview(c, captures, index, next, variables, work)?);
            }
            if repeated {
                let term = &p[p.len() - 2];
                let mut pending = vec![term];
                let mut length = 0;
                while let Some(n) = pending.pop() {
                    if let Some(Capture::Many(v)) = n.atom().and_then(|n| captures.get(n)) {
                        length = v.len();
                    }
                    if let Some(p) = n.list() {
                        pending.extend(p);
                    }
                }
                for i in 0..length {
                    parts.push(preview(term, captures, Some(i), next, variables, work)?);
                }
            }
            out.kind = Kind::List(parts);
        }
        Ok(out)
    }
    let Some(target) = node
        .head()
        .and_then(|h| h.strip_prefix('%'))
        .and_then(|n| definitions.get(n))
    else {
        return Ok(BTreeSet::new());
    };
    let mut variables = BTreeMap::new();
    let mut remaining = budget;
    let preview = preview(
        node,
        captures,
        index,
        &mut 1,
        &mut variables,
        &mut remaining,
    )
    .map_err(|()| {
        error(source, node, "delegated binding preview limit exceeded").with_code("E_MACRO_LIMIT")
    })?;
    let mut allowed = BTreeSet::new();
    let interface = target.binding_inputs(&preview);
    if interface.is_none() && target.rules.iter().any(|r| !r.bindings.is_empty()) {
        return Err(error(
            source,
            node,
            "delegated binding interface must agree across matching rules",
        )
        .with_code("E_MACRO_BINDING"));
    }
    for (binder, body) in interface.unwrap_or_default() {
        let Some((binder, _, binder_index)) = variables.get(&binder.context.id) else {
            continue;
        };
        let Some((body, span, body_index)) = variables.get(&body.context.id) else {
            continue;
        };
        if !rule.bindings.contains(&(binder.clone(), body.clone())) {
            return Err(error(
                source,
                node,
                "delegated caller binding requires the same bind clause in the enclosing rule",
            )
            .with_code("E_MACRO_BINDING"));
        }
        allowed.insert(Delegated {
            binder: binder.clone(),
            body: body.clone(),
            occurrence: *span,
            index: *body_index,
            binder_index: *binder_index,
        });
    }
    Ok(allowed)
}
