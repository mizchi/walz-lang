//! Call-boundary inference. This produces ordinary applications; it cannot
//! manufacture IR or discharge constraints/effects/ownership obligations.
use super::*;
use std::cell::{Cell, RefCell};

pub(crate) struct Inference {
    functions: BTreeMap<String, Template>,
    aliases: BTreeMap<String, Alias>,
    concrete: BTreeMap<String, Sexp>,
    instances: BTreeMap<(ReferenceKind, String, Vec<String>), String>,
    resolved: RefCell<BTreeMap<usize, Resolution>>,
    pending: RefCell<Option<Request>>,
    type_work: Cell<usize>,
}

struct Alias {
    parameters: Vec<String>,
    target: Sexp,
}
struct Resolution {
    node: Sexp,
    concrete: String,
    arguments: Vec<Sexp>,
}
struct Request {
    node: usize,
    application: Sexp,
    caller: String,
}

pub(crate) struct Call<'a> {
    context: &'a Inference,
    template: &'a Template,
    pub params: Vec<Sexp>,
    pub result: Sexp,
    pub callable: Vec<Sexp>,
    bindings: BTreeMap<String, Sexp>,
}

fn substitute(node: &Sexp, bindings: &BTreeMap<String, Sexp>) -> Sexp {
    if let Some(value) = node.atom().and_then(|n| bindings.get(n)) {
        return value.clone();
    }
    let mut out = node.clone();
    if let Kind::List(items) = &mut out.kind {
        for item in items {
            *item = substitute(item, bindings);
        }
    }
    out
}

impl Inference {
    pub fn call(&self, name: &str) -> Option<Call<'_>> {
        let template = self.functions.get(name)?;
        let p = template.syntax.list().unwrap();
        let params = p
            .iter()
            .filter(|n| n.head() == Some("param"))
            .flat_map(|n| {
                let p = n.list().unwrap();
                // Named and grouped unnamed WAT parameter declarations.
                let start = if p
                    .get(1)
                    .and_then(Sexp::atom)
                    .is_some_and(|n| n.starts_with('$'))
                {
                    2
                } else {
                    1
                };
                p[start..].to_vec()
            })
            .collect();
        let result = p
            .iter()
            .find(|n| n.head() == Some("result"))
            .and_then(|n| n.list()?.get(1))
            .cloned()
            .unwrap_or_else(|| Sexp {
                kind: Kind::Atom("unit".into()),
                span: template.syntax.span,
                context: Default::default(),
            });
        let callable = p
            .iter()
            .find(|n| n.head() == Some("where"))
            .and_then(Sexp::list)
            .into_iter()
            .flatten()
            .filter(|n| n.head() == Some("callable"))
            .cloned()
            .collect();
        Some(Call {
            context: self,
            template,
            params,
            result,
            callable,
            bindings: BTreeMap::new(),
        })
    }
    /// Resolve only cached concrete applications. A missing specialization
    /// invalidates the headers and is handled by the outer checking session.
    pub fn resolve_existing(&self, node: &mut Sexp) -> Result<bool, Error> {
        fn apply(
            context: &Inference,
            kind: ReferenceKind,
            n: &mut Sexp,
            complete: &mut bool,
        ) -> Result<(), Error> {
            if let Some(parts) = n
                .list()
                .filter(|_| n.head().is_some_and(|h| h.starts_with('$')))
            {
                let mut arguments = parts[1..].to_vec();
                for a in &mut arguments {
                    match context.canonical(a, 0) {
                        Ok(canonical) => *a = canonical,
                        Err(_) => {
                            *complete = false;
                            return Ok(());
                        }
                    }
                    type_syntax::map_type(a, &mut |a| {
                        apply(context, ReferenceKind::Type, a, complete)
                    })?;
                }
                let keys = arguments.iter().map(key).collect();
                if let Some(concrete) =
                    context
                        .instances
                        .get(&(kind, n.head().unwrap().into(), keys))
                {
                    n.kind = Kind::Atom(concrete.clone());
                } else {
                    *complete = false;
                }
            }
            Ok(())
        }
        let mut complete = true;
        type_syntax::map_types(node, &mut |n| {
            apply(self, ReferenceKind::Type, n, &mut complete)
        })?;
        type_syntax::map_references(node, &mut |kind, n| apply(self, kind, n, &mut complete))?;
        Ok(complete)
    }
    fn canonical(&self, node: &Sexp, depth: usize) -> Result<Sexp, String> {
        let work = self.type_work.get();
        if depth > 64 || work >= 1_000_000 {
            return Err("generic type inference depth/work limit exceeded".into());
        }
        self.type_work.set(work + 1);
        if matches!(node.kind, Kind::Text(_) | Kind::Bytes(_)) {
            return Err("expected type syntax, found string literal".into());
        }
        let name = node.atom().or_else(|| node.head()).unwrap_or("");
        if let Some(alias) = self.aliases.get(name) {
            let args = node.list().map(|p| &p[1..]).unwrap_or(&[]);
            if args.len() == alias.parameters.len() {
                let bindings = alias
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                return self.canonical(&substitute(&alias.target, &bindings), depth + 1);
            }
        }
        let mut out = node.clone();
        if let Kind::List(parts) = &mut out.kind {
            for part in parts.iter_mut().skip(1) {
                *part = self.canonical(part, depth + 1)?;
            }
        }
        Ok(out)
    }
    pub fn apply_resolved(&self, expanded: &mut Expanded) {
        fn walk(n: &mut Sexp, resolutions: &BTreeMap<usize, Resolution>) {
            if let Some(resolution) = resolutions.get(&n.context.id) {
                n.kind = Kind::Atom(resolution.concrete.clone());
                return;
            }
            if let Kind::List(p) = &mut n.kind {
                for c in p {
                    walk(c, resolutions);
                }
            }
        }
        let resolved = self.resolved.borrow();
        walk(&mut expanded.syntax, &resolved);
        for resolution in resolved.values() {
            let Resolution {
                node,
                concrete: name,
                arguments: args,
            } = resolution;
            let generic = node.atom().unwrap();
            expanded.instances.push((
                self.functions[generic].syntax.span,
                node.span,
                format!(
                    "{generic}<{}>",
                    args.iter().map(key).collect::<Vec<_>>().join(", ")
                ),
                name.clone(),
            ));
        }
    }
}
impl Call<'_> {
    pub fn is_complete(&self) -> bool {
        self.template
            .parameters
            .iter()
            .all(|n| self.bindings.contains_key(n))
    }

    pub fn error(&self, source: &str, node: &Sexp, message: impl std::fmt::Display) -> Error {
        Error::at(source,node.span,message).with_code("E_GENERIC_INFERENCE")
            .related(source,self.template.syntax.span,"generic declaration")
            .help("specify type arguments with (call (%inst $function type...) arguments...) or add a type annotation; numeric widths are never guessed")
    }
    pub fn hint(&self, pattern: &Sexp) -> Result<Option<Sexp>, String> {
        let out = self
            .context
            .canonical(&substitute(pattern, &self.bindings), 0)?;
        fn unresolved(n: &Sexp, params: &[String]) -> bool {
            n.atom().is_some_and(|n| params.iter().any(|p| p == n))
                || n.list()
                    .is_some_and(|p| p.iter().any(|n| unresolved(n, params)))
        }
        if unresolved(&out, &self.template.parameters) {
            Ok(None)
        } else {
            Ok(Some(out))
        }
    }
    pub fn unify(&mut self, pattern: &Sexp, actual: &Sexp) -> Result<(), String> {
        self.unify_at(pattern, actual, 0)
    }
    fn unify_at(&mut self, pattern: &Sexp, actual: &Sexp, depth: usize) -> Result<(), String> {
        if depth > 64 {
            return Err("type inference depth limit exceeded".into());
        }
        if let Some(name) = pattern
            .atom()
            .filter(|n| self.template.parameters.iter().any(|p| p == *n))
        {
            if let Some(previous) = self.bindings.get(name) {
                if key(previous) != key(actual) {
                    return Err(format!(
                        "conflicting types for `{name}`: {} and {}",
                        key(previous),
                        key(actual)
                    ));
                }
            } else {
                self.bindings.insert(name.into(), actual.clone());
            }
            return Ok(());
        }
        let p = self.context.canonical(pattern, 0)?;
        let a = self.context.canonical(actual, 0)?;
        if key(&p) != key(pattern) || key(&a) != key(actual) {
            return self.unify_at(&p, &a, depth + 1);
        }
        if let Some(reified) = a.atom().and_then(|n| self.context.concrete.get(n))
            && p.list().is_some()
        {
            return self.unify_at(&p, reified, depth + 1);
        }
        if let (Some(p), Some(a)) = (p.list(), a.list()) {
            // A nullable parameter accepts a non-null argument, without inventing
            // a nullable common type for two conflicting inferred variables.
            if p.len() == 3
                && p[0].atom() == Some("ref")
                && p[1].atom() == Some("null")
                && a.len() == 2
                && a[0].atom() == Some("ref")
            {
                return self.unify_at(&p[2], &a[1], depth + 1);
            }
            if p.len() == a.len()
                && p.first().and_then(Sexp::atom) == a.first().and_then(Sexp::atom)
            {
                for (p, a) in p.iter().skip(1).zip(a.iter().skip(1)) {
                    self.unify_at(p, a, depth + 1)?;
                }
                return Ok(());
            }
        }
        if key(&p) == key(&a) {
            Ok(())
        } else {
            Err(format!("cannot unify {} with {}", key(&p), key(&a)))
        }
    }
    pub fn finish(self, source: &str, callee: &Sexp, caller: &str) -> Result<String, Error> {
        let args = self
            .template
            .parameters
            .iter()
            .map(|n| {
                self.bindings.get(n).cloned().ok_or_else(|| {
                    self.error(
                        source,
                        callee,
                        format!("cannot uniquely infer type parameter `{n}`"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let name = callee.atom().unwrap();
        let keys = args.iter().map(key).collect();
        if let Some(concrete) =
            self.context
                .instances
                .get(&(ReferenceKind::Function, name.into(), keys))
        {
            self.context.resolved.borrow_mut().insert(
                callee.context.id,
                Resolution {
                    node: callee.clone(),
                    concrete: concrete.clone(),
                    arguments: args,
                },
            );
            return Ok(concrete.clone());
        }
        let mut application = callee.clone();
        application.kind = Kind::List(std::iter::once(callee.clone()).chain(args).collect());
        *self.context.pending.borrow_mut() = Some(Request {
            node: callee.context.id,
            application,
            caller: caller.into(),
        });
        Err(
            Error::at(source, callee.span, "generic specialization pending")
                .with_code("E_GENERIC_PENDING"),
        )
    }
}

impl Session<'_> {
    pub fn has_templates(&self) -> bool {
        !self.0.templates.is_empty()
    }
    pub fn charge_check(&mut self, root: &Sexp) -> Result<(), Error> {
        if !self.has_templates() {
            return Ok(());
        }
        let mut pending = vec![root];
        while let Some(n) = pending.pop() {
            self.0.check_work += 1;
            if self.0.check_work > 2_000_000 {
                return Err(self.0.error(n, "generic elaboration work limit exceeded"));
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        Ok(())
    }
    pub fn inference(&self) -> Inference {
        let e = &self.0;
        let functions = e
            .templates
            .iter()
            .filter(|t| t.syntax.head() == Some("func"))
            .map(|t| {
                (
                    t.syntax.list().unwrap()[1].atom().unwrap().into(),
                    t.clone(),
                )
            })
            .collect();
        let mut aliases = e
            .templates
            .iter()
            .filter(|t| t.syntax.head() == Some("type.alias"))
            .map(|t| {
                (
                    t.syntax.list().unwrap()[1].atom().unwrap().into(),
                    Alias {
                        parameters: t.parameters.clone(),
                        target: t.syntax.list().unwrap().last().unwrap().clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (name, target) in &e.aliases {
            aliases.insert(
                name.clone(),
                Alias {
                    parameters: vec![],
                    target: target.clone(),
                },
            );
        }
        let mut concrete = BTreeMap::new();
        let mut instances = BTreeMap::new();
        for ((index, keys), name) in &e.cache {
            let template = &e.templates[*index];
            let generic = template.syntax.list().unwrap()[1].atom().unwrap();
            let parts = std::iter::once(generic.into())
                .chain(keys.iter().cloned())
                .collect::<Vec<String>>()
                .join(" ");
            concrete.insert(
                name.clone(),
                crate::sexpr::parse(&format!("({parts})")).expect("canonical type"),
            );
            for ((kind, definition), target) in &e.definitions {
                if target == index {
                    instances.insert((*kind, definition.clone(), keys.clone()), name.clone());
                }
            }
        }
        Inference {
            functions,
            aliases,
            concrete,
            instances,
            resolved: RefCell::default(),
            pending: RefCell::default(),
            type_work: Cell::new(0),
        }
    }
    pub fn resume_inference(
        &mut self,
        expanded: &mut Expanded,
        inference: &Inference,
    ) -> Result<(), Error> {
        inference.apply_resolved(expanded);
        self.0.instances.clone_from(&expanded.instances);
        let Request {
            node: id,
            application: node,
            caller,
        } = inference
            .pending
            .borrow_mut()
            .take()
            .expect("pending generic request");
        self.0.active = self.0.ancestries.get(&caller).cloned().unwrap_or_default();
        let concrete = self.0.apply(ReferenceKind::Function, &node)?;
        fn replace(n: &mut Sexp, id: usize, value: &Sexp) -> bool {
            if n.context.id == id {
                *n = value.clone();
                return true;
            }
            if let Kind::List(p) = &mut n.kind {
                for c in p {
                    if replace(c, id, value) {
                        return true;
                    }
                }
            }
            false
        }
        assert!(
            replace(&mut expanded.syntax, id, &concrete),
            "inference call must belong to current syntax"
        );
        self.refresh(expanded)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_inferred_call_sites_survive_a_later_specialization_restart() {
        let source = "(module (func $id (%type.params $T) (param $x $T) (result $T) $x) (func $f (block (drop (call $id (i64.const 1))) (drop (call $id (i64.const 2))) (drop (call $id (i32.const 3))))))";
        let (_, instances) =
            crate::check_observed(source, &Default::default(), |_, g| g.instances.clone()).unwrap();
        assert_eq!(instances.len(), 3, "{instances:?}");
    }
}
