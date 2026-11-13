//! Closed, compile-time Component constructors. Specialization shares a
//! definition, never an instance, memory or nominal resource identity.
use super::*;
use crate::ir::Span;
mod arguments;
mod snapshot;
mod stage;
mod substitute;

const MAX_INSTANCES: usize = 1024;
const MAX_NODES: usize = 1_000_000;
#[derive(Default)]
pub(super) struct Budget {
    instances: usize,
    nodes: usize,
    argument_bytes: usize,
}
impl Budget {
    fn charge(&mut self, source: &str, n: &Sexp) -> Result<(), Error> {
        let mut pending = vec![n];
        while let Some(n) = pending.pop() {
            self.nodes += 1;
            if self.nodes > MAX_NODES {
                return Err(fail(
                    source,
                    n,
                    "Component specialization node limit exceeded",
                ));
            }
            if let Some(p) = n.list() {
                pending.extend(p);
            }
        }
        Ok(())
    }
}
fn fail(source: &str, node: &Sexp, message: impl std::fmt::Display) -> Error {
    Error::at(source, node.span, message).with_code("E_COMPONENT_GENERIC")
}
struct Template {
    syntax: Sexp,
    parameters: Vec<String>,
}
pub(super) struct Expanded {
    pub text: String,
    origins: Vec<(Span, Span)>,
    requests: Vec<(Span, Span)>,
    names: BTreeMap<String, String>,
    type_names: Vec<(Span, BTreeMap<String, String>)>,
}
impl Expanded {
    pub fn annotate(&self, source: &str, mut e: Error) -> Error {
        let at = e.diagnostic.primary.as_ref().map(|l| l.span.start);
        for label in e
            .diagnostic
            .primary
            .iter_mut()
            .chain(&mut e.diagnostic.related)
        {
            if label.location.path.is_some() {
                continue;
            }
            if let Some((_, span)) = self
                .origins
                .iter()
                .find(|(r, _)| r.start <= label.span.start && label.span.start < r.end)
            {
                *label = crate::diagnostic::Label::new(source, *span, &label.message);
            }
        }
        if let Some(at) = at {
            for (_, names) in self
                .type_names
                .iter()
                .filter(|(r, _)| r.start <= at && at < r.end)
            {
                e = e.rewrite_names(names);
            }
            for (_, request) in self
                .requests
                .iter()
                .filter(|(r, _)| r.start <= at && at < r.end)
                .take(8)
            {
                e = e.related(source, *request, "Component specialization requested here");
            }
        }
        e.rewrite_names(&self.names)
    }
    fn write(&mut self, node: &Sexp) {
        let start = self.text.len();
        if let Some(p) = node.list() {
            self.text.push('(');
            for (i, n) in p.iter().enumerate() {
                if i != 0 {
                    self.text.push(' ');
                }
                self.write(n);
            }
            self.text.push(')');
        } else {
            self.text.push_str(&render(node));
        }
        // Children precede their enclosing ranges so the narrowest origin wins.
        self.origins.push((
            Span {
                start,
                end: self.text.len(),
            },
            node.span,
        ));
    }
}

/// Native names in the Component namespace take precedence over angle sugar.
fn names(fields: &[Sexp]) -> BTreeSet<String> {
    fields
        .iter()
        .filter_map(|n| {
            let p = n.list()?;
            let descriptor = match n.head()? {
                "component" | "@component.constructor" => n,
                "import" => p.get(2)?,
                "alias" => p.last()?,
                _ => return None,
            };
            if !matches!(
                descriptor.head(),
                Some("component" | "@component.constructor")
            ) {
                return None;
            }
            descriptor.list()?.get(1)?.atom().map(str::to_owned)
        })
        .collect()
}
fn parameters(source: &str, node: &Sexp) -> Result<Vec<String>, Error> {
    let mut header = crate::applications::canonical_view(node);
    let Kind::List(p) = &mut header.kind else {
        return Ok(vec![]);
    };
    p[0].kind = Kind::Atom("func".into());
    crate::generics::parameters(source, &header).map_err(|e| {
        let d = e.diagnostic();
        Error::at(
            source,
            d.primary.as_ref().map_or(node.span, |l| l.span),
            &d.message,
        )
        .with_code("E_COMPONENT_GENERIC")
    })
}
pub(super) fn expand(
    source: &str,
    root: &Sexp,
    budget: &mut Budget,
    policy: &BindingPolicy,
    depth: usize,
) -> Result<Option<Expanded>, Error> {
    let fields = root.list().unwrap();
    if fields.iter().any(|n| n.head() == Some("@type.params")) {
        return Err(fail(
            source,
            root,
            "a generic Component is a constructor; select it with @use and supply explicit type arguments to instantiate",
        ));
    }
    let mut templates = BTreeMap::new();
    let mut declared = BTreeSet::new();
    let mut changed = false;
    for n in fields
        .iter()
        .skip(1)
        .filter(|n| n.head() == Some("component"))
    {
        let params = parameters(source, n)?;
        if params.is_empty() {
            continue;
        }
        let name = n
            .list()
            .unwrap()
            .get(1)
            .and_then(Sexp::atom)
            .filter(|s| crate::sexpr::identifier(s))
            .ok_or_else(|| {
                fail(
                    source,
                    n,
                    "a generic Component constructor requires a $name",
                )
            })?;
        if templates
            .insert(
                name.to_owned(),
                Template {
                    syntax: n.clone(),
                    parameters: params,
                },
            )
            .is_some()
        {
            return Err(fail(source, n, "duplicate generic Component constructor"));
        }
        changed = true;
    }
    let mut native_names = names(&fields[1..]);
    for name in templates.keys() {
        native_names.remove(name);
    }
    for n in &fields[1..] {
        if let Some(name) = names(std::slice::from_ref(n)).first()
            && !declared.insert(name.clone())
        {
            return Err(fail(source, n, "duplicate Component constructor name"));
        }
    }
    let environment = arguments::environment(root);
    let stage = stage::Context {
        source,
        root,
        policy,
        depth,
    };
    let mut cache = BTreeMap::<(String, Vec<String>), String>::new();
    let mut arguments = BTreeMap::<(String, Vec<String>), snapshot::Snapshot>::new();
    let mut pending = vec![];
    let mut output = Expanded {
        text: String::new(),
        origins: vec![],
        requests: vec![],
        names: BTreeMap::new(),
        type_names: vec![],
    };
    let mut rewritten = fields[..1].to_vec();
    if let Some(n) = fields.get(1).filter(|n| n.atom().is_some()) {
        rewritten.push(n.clone());
    }
    let start = if fields.get(1).is_some_and(|n| n.atom().is_some()) {
        2
    } else {
        1
    };
    let mut requests = vec![];
    for n in fields.iter().skip(start) {
        if n.head() == Some("component") && !parameters(source, n)?.is_empty() {
            continue;
        }
        let mut n = n.clone();
        let mut specialize = |target: &mut Sexp| -> Result<(), Error> {
            if target.atom().is_some_and(|n| native_names.contains(n)) {
                return Ok(());
            }
            let parsed = application(source, target)?;
            let (name, args) =
                if let Some(p) = parsed.list().filter(|_| parsed.head() == Some("@inst")) {
                    (
                        p.get(1).and_then(Sexp::atom).unwrap_or(""),
                        &p[2.min(p.len())..],
                    )
                } else if let Some(name) = parsed.atom().filter(|n| templates.contains_key(*n)) {
                    (name, &[][..])
                } else {
                    return Ok(());
                };
            let template = templates.get(name).ok_or_else(|| {
                fail(
                    source,
                    target,
                    format!("`{name}` is not a generic Component constructor"),
                )
            })?;
            if args.len() != template.parameters.len() {
                return Err(fail(
                    source,
                    target,
                    format!(
                        "generic Component `{name}` requires {} explicit type arguments, got {}",
                        template.parameters.len(),
                        args.len()
                    ),
                )
                .related(source, template.syntax.span, "constructor declared here"));
            }
            let raw_key = (name.to_owned(), args.iter().map(render).collect::<Vec<_>>());
            let display = format!("{name}<{}>", raw_key.1.join(", "));
            let args = if let Some(args) = arguments.get(&raw_key) {
                args.clone()
            } else {
                let args = arguments::resolve(
                    source,
                    target,
                    &environment,
                    template,
                    args,
                    budget,
                    &mut |namespaces, budget| {
                        stage.connections(target, &rewritten, namespaces, budget)
                    },
                )?;
                arguments.insert(raw_key, args.clone());
                args
            };
            let key = args.key.clone();
            let concrete = if let Some(name) = cache.get(&(name.to_owned(), key.clone())) {
                name.clone()
            } else {
                budget.instances += 1;
                if budget.instances > MAX_INSTANCES {
                    return Err(fail(
                        source,
                        target,
                        "Component specialization count limit exceeded",
                    ));
                }
                let mut suffix = cache.len();
                let concrete = loop {
                    let candidate = format!("{name}.mono{suffix}");
                    if !source.contains(&candidate) && declared.insert(candidate.clone()) {
                        break candidate;
                    }
                    suffix += 1;
                };
                let mut body = template.syntax.clone();
                let Kind::List(parts) = &mut body.kind else {
                    unreachable!()
                };
                parts[1].kind = Kind::Atom(concrete.clone());
                parts.retain(|n| !matches!(n.head(), Some("@type.params" | "@where")));
                let bindings = template
                    .parameters
                    .iter()
                    .cloned()
                    .zip(args.arguments)
                    .collect();
                substitute::apply(source, &mut body, &bindings)?;
                snapshot::inject(&mut body, &args.declarations);
                budget.charge(source, &body)?;
                cache.insert((name.to_owned(), key.clone()), concrete.clone());
                output.names.insert(concrete.clone(), display);
                pending.push((body, target.span, args.labels));
                concrete
            };
            target.kind = Kind::Atom(concrete);
            changed = true;
            Ok(())
        };
        references(&mut n, &mut specialize)?;
        for (body, request, labels) in pending.drain(..) {
            // Record requests after rendering below so nested diagnostics map
            // back to both the definition and its application.
            let slot = rewritten.len();
            rewritten.push(body);
            requests.push((slot, request, labels));
        }
        rewritten.push(n);
    }
    if !changed {
        return Ok(None);
    }
    // Inserting definitions changes the native Component index space. Keep
    // native code unchanged, and reject ambiguous numeric references in an
    // extended scope rather than silently redirecting them to a specialization.
    for n in fields.iter().skip(1) {
        let mut n = n.clone();
        references(&mut n, &mut |target| {
            if target.atom().is_some_and(|n| !n.starts_with('$')) {
                return Err(fail(
                    source,
                    target,
                    "use named Component references in a scope with generic constructors; specialization changes numeric Component indices",
                ));
            }
            Ok(())
        })?;
        check_outer_indices(source, &n)?;
    }
    output.text.push_str("(component ");
    for (slot, n) in rewritten.iter().enumerate().skip(1) {
        let start = output.text.len();
        output.write(n);
        let end = output.text.len();
        for (_, request, labels) in requests.iter().filter(|(i, _, _)| *i == slot) {
            output.requests.push((Span { start, end }, *request));
            output
                .type_names
                .push((Span { start, end }, labels.clone()));
        }
        output.text.push('\n');
    }
    output.text.push(')');
    Ok(Some(output))
}
fn check_outer_indices(source: &str, n: &Sexp) -> Result<(), Error> {
    if n.head().is_some_and(crate::syntax::annotation) {
        return Ok(());
    }
    let Some(p) = n.list() else { return Ok(()) };
    if n.head() == Some("alias")
        && p.get(1).and_then(Sexp::atom) == Some("outer")
        && p.last().and_then(Sexp::head) == Some("component")
        && let Some(index) = p
            .get(3)
            .filter(|n| n.atom().is_some_and(|n| !n.starts_with('$')))
    {
        return Err(fail(
            source,
            index,
            "use a named Component in outer aliases when generic constructors are present",
        ));
    }
    for child in p.iter().skip(1) {
        check_outer_indices(source, child)?;
    }
    Ok(())
}
fn application(source: &str, target: &Sexp) -> Result<Sexp, Error> {
    if target
        .atom()
        .is_some_and(|s| crate::sexpr::applications::prefix(s).is_some())
    {
        crate::sexpr::applications::parse(source, target, &BTreeSet::new())
    } else {
        Ok(target.clone())
    }
}
fn references(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    let head = node.head().unwrap_or("").to_owned();
    if matches!(head.as_str(), "component" | "core" | "@lib") || crate::syntax::annotation(&head) {
        return Ok(());
    }
    let Kind::List(p) = &mut node.kind else {
        return Ok(());
    };
    if head == "instantiate" {
        if let Some(target) = p.get_mut(1) {
            f(target)?;
        }
        return Ok(());
    }
    if head == "export"
        && let Some(descriptor) = p.get_mut(2)
        && descriptor.head() == Some("component")
    {
        if let Kind::List(p) = &mut descriptor.kind
            && let Some(target) = p.get_mut(1)
        {
            f(target)?;
        }
        return Ok(());
    }
    for n in p.iter_mut().skip(1) {
        references(n, f)?;
    }
    Ok(())
}
