//! Resolve arguments in the caller's static type scope with the ordinary Walz
//! checker. Closed structural types are descriptions, not foreign heap IDs.
use super::*;
use crate::{
    component::bound_imports::BoundImports,
    ir::{Module, Type, TypeKind, ValueType},
};

pub(super) fn environment(root: &Sexp) -> String {
    fn types(n: &Sexp) -> Option<String> {
        let p = n.list()?;
        Some(match n.head()? {
            "type" | "@type.alias" | "@abi.type" => render(n),
            "@export" => {
                let selected = p
                    .iter()
                    .skip(1)
                    .filter(|n| matches!(n.head(), Some("type" | "opaque")))
                    .map(render)
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    return None;
                }
                format!("(@export {})", selected.join(" "))
            }
            "@use" | "@file.use" => {
                let selected = p
                    .get(2..)?
                    .iter()
                    .filter(|n| n.head() == Some("type"))
                    .map(render)
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    return None;
                }
                format!(
                    "({} {} {})",
                    render(&p[0]),
                    render(&p[1]),
                    selected.join(" ")
                )
            }
            "component" | "@lib" | "@file.namespace" => {
                let skip = if n.head() == Some("@file.namespace") {
                    2
                } else {
                    1
                };
                let body = p
                    .iter()
                    .skip(skip)
                    .filter_map(types)
                    .collect::<Vec<_>>()
                    .join("\n");
                if n.head() == Some("@file.namespace") {
                    format!("(@file.namespace {} {body})", render(&p[1]))
                } else {
                    body
                }
            }
            _ => return None,
        })
    }
    // Nested Component definitions have their own static type scope.
    root.list()
        .unwrap()
        .iter()
        .skip(1)
        .filter(|n| {
            matches!(
                n.head(),
                Some("@lib" | "@use" | "@file.use" | "@file.namespace")
            )
        })
        .filter_map(types)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn resolve(
    source: &str,
    request: &Sexp,
    environment: &str,
    template: &Template,
    args: &[Sexp],
    budget: &mut Budget,
    connections: &mut impl FnMut(&BTreeSet<String>, &mut Budget) -> Result<BoundImports, Error>,
) -> Result<super::snapshot::Snapshot, Error> {
    let mut prefix = "$walz-constructor-argument".to_owned();
    while source.contains(&prefix) {
        prefix.push('x');
    }
    let mut probe = environment.to_owned();
    for (i, arg) in args.iter().enumerate() {
        probe.push_str(&format!(
            "(func {prefix}{i} (result (@option {})) (unreachable))\n",
            render(arg)
        ));
    }
    let env = crate::composition::queries::deferred::environment(probe, source)?;
    let charge = |text: &str, budget: &mut Budget| -> Result<(), Error> {
        budget.argument_bytes = budget.argument_bytes.saturating_add(text.len());
        if budget.argument_bytes > 16 * 1024 * 1024 {
            return Err(fail(
                source,
                request,
                "Component argument checking size limit exceeded",
            ));
        }
        Ok(())
    };
    let map_error = |e: Error| {
        let e = e.rewrite_names(&BTreeMap::from([(
            format!("{prefix}-bound"),
            template.syntax.list().unwrap()[1]
                .atom()
                .unwrap()
                .to_owned(),
        )]));
        let d = e.diagnostic();
        Error::at(source, request.span, &d.message)
            .with_code(
                if matches!(
                    d.code,
                    "E_GENERIC_BOUND" | "E_COMPONENT_TYPE" | "E_COMPONENT_TYPE_PHASE"
                ) {
                    d.code
                } else {
                    "E_COMPONENT_GENERIC"
                },
            )
            .related(source, template.syntax.span, "constructor declared here")
    };
    let argument_types = |m: &Module| -> Vec<Type> {
        (0..args.len())
            .map(|i| {
                let f = m
                    .functions
                    .iter()
                    .find(|f| f.name == format!("{prefix}{i}"))
                    .unwrap();
                let Type::Value(id) = f.result else {
                    unreachable!()
                };
                let TypeKind::Value(ValueType::Option(t)) = &m.types[id.0].kind else {
                    unreachable!()
                };
                t.clone()
            })
            .collect()
    };
    let text = format!("(module {})", env.source);
    charge(&text, budget)?;
    let discovered = crate::check(&text).map_err(map_error)?;
    let needed = env.needed(&discovered, &argument_types(&discovered));
    let mut origins = BTreeMap::new();
    let actual = if needed.is_empty() {
        env.source
    } else {
        let namespaces = needed
            .iter()
            .filter_map(|n| env.queries[n].list()?.get(1)?.text().map(str::to_owned))
            .collect();
        let bound = connections(&namespaces, budget)?;
        let (actual, types) = env.resolve(&needed, &bound).map_err(map_error)?;
        for (name, ty) in types {
            origins.insert(
                name,
                format!(
                    "wit:{}",
                    crate::wit_bindgen::contract_shape(&bound.resolve, ty).map_err(map_error)?
                ),
            );
        }
        actual
    };
    // Constraints see real types, never the discovery witnesses.
    let headers = template
        .syntax
        .list()
        .unwrap()
        .iter()
        .filter(|n| matches!(n.head(), Some("@type.params" | "@where")))
        .map(render)
        .collect::<Vec<_>>()
        .join(" ");
    // Refer to resolved argument signatures through local aliases, avoiding a
    // second occurrence of a direct connection query in the bound invocation.
    let syntax = crate::sexpr::parse(&format!("(module {actual})"))?;
    let mut aliases = String::new();
    let mut names = vec![];
    for i in 0..args.len() {
        let f = syntax
            .list()
            .unwrap()
            .iter()
            .find(|n| {
                n.head() == Some("func")
                    && n.list().unwrap().get(1).and_then(Sexp::atom)
                        == Some(format!("{prefix}{i}").as_str())
            })
            .unwrap();
        let result = f
            .list()
            .unwrap()
            .iter()
            .find(|n| n.head() == Some("result"))
            .unwrap();
        let ty = &result.list().unwrap()[1].list().unwrap()[1];
        let name = format!("{prefix}-type{i}");
        aliases += &format!("(@type.alias {name} {})\n", render(ty));
        names.push(name);
    }
    let type_text = names.join(" ");
    let text = format!(
        "(module {actual} {aliases} (func {prefix}-bound {headers} (nop)) (func {prefix}-check (call (@inst {prefix}-bound {type_text}))))"
    );
    charge(&text, budget)?;
    let (checked, (generic_origins, generic_arguments)) = crate::check_observed_with(
        &text,
        &Default::default(),
        |_, g| (g.origins.clone(), g.arguments.clone()),
        |_| Ok(()),
    )
    .map_err(map_error)?;
    let syntax = crate::sexpr::parse(&text)?;
    let standard = crate::standard::prepare(&text, &syntax)?;
    let syntax = standard.syntax(syntax)?;
    let origin_source = standard.source(&text);
    let mut syntax = crate::syntax::normalize(origin_source, &syntax)?;
    standard.rewrite(&mut syntax);
    let mut opaque = crate::modules::expand(origin_source, &syntax)?.opaque_names();
    let mut identities = Identity {
        origins: &generic_origins,
        arguments: &generic_arguments,
        queries: &origins,
        visits: 0,
        bytes: 0,
    };
    let mut nominal_origins = BTreeMap::new();
    for decl in &checked.types {
        if generic_arguments.contains_key(&decl.name) {
            let key = identities.name(&decl.name, 0).ok_or_else(|| {
                fail(
                    source,
                    request,
                    "Component nominal argument identity limit exceeded",
                )
            })?;
            nominal_origins.insert(decl.name.clone(), format!("source:{key}"));
        }
    }
    origins.extend(nominal_origins);
    for (name, origin) in generic_origins {
        if opaque.contains(&origin) {
            opaque.insert(name);
        }
    }
    super::snapshot::build(
        source,
        request,
        &checked,
        &argument_types(&checked),
        &origins,
        &opaque,
    )
}

// Preserve semantic applications through a structured side table, never through
// diagnostic displays or generated ordinal spellings. Phantom arguments matter.
struct Identity<'a> {
    origins: &'a BTreeMap<String, String>,
    arguments: &'a BTreeMap<String, Vec<Sexp>>,
    queries: &'a BTreeMap<String, String>,
    visits: usize,
    bytes: usize,
}
impl Identity<'_> {
    fn string(&mut self, s: &str) -> Option<serde_json::Value> {
        self.bytes = self.bytes.saturating_add(s.len());
        (self.bytes <= 16 * 1024 * 1024).then(|| serde_json::Value::String(s.to_owned()))
    }
    fn name(&mut self, name: &str, depth: usize) -> Option<serde_json::Value> {
        self.visits += 1;
        if depth > 64 || self.visits > 65536 {
            return None;
        }
        let mut key = vec![];
        if let Some(args) = self.arguments.get(name) {
            key.push(self.string("application")?);
            key.push(self.string(self.origins.get(name)?)?);
            for arg in args {
                key.push(self.ty(arg, depth + 1)?);
            }
        } else if let Some(origin) = self.queries.get(name) {
            key.push(self.string("wit")?);
            key.push(self.string(origin)?);
        } else {
            key.push(self.string("name")?);
            key.push(self.string(name)?);
        }
        Some(serde_json::Value::Array(key))
    }
    fn ty(&mut self, n: &Sexp, depth: usize) -> Option<serde_json::Value> {
        if let Some(name) = n.atom() {
            return self.name(name, depth);
        }
        self.visits += 1;
        if depth > 64 || self.visits > 65536 {
            return None;
        }
        let mut key = vec![self.string("syntax")?];
        for n in n.list()? {
            key.push(self.ty(n, depth + 1)?);
        }
        Some(serde_json::Value::Array(key))
    }
}
