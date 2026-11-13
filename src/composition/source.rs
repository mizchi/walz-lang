//! Compile-time syntax only. Native definitions and their instance order are
//! retained; library calls share one checked implementation, never a new instance
//! per use. Component-to-component calls retain canonical value conversion.
use super::bindings::{Bind, InstanceBind};
use super::*;

struct Text {
    source: String,
    origins: Vec<(std::ops::Range<usize>, crate::ir::Span)>,
}
impl Text {
    fn new() -> Self {
        Self {
            source: "(module\n".into(),
            origins: vec![],
        }
    }
    fn push(&mut self, text: &str, span: crate::ir::Span) {
        let start = self.source.len();
        self.source.push_str(text);
        self.source.push('\n');
        self.origins.push((start..self.source.len(), span));
    }
    fn diagnostic(&self, original: &str, e: Error) -> Error {
        let d = e.diagnostic();
        if let Some(p) = &d.primary
            && let Some((_, span)) = self
                .origins
                .iter()
                .find(|(range, _)| range.contains(&p.span.start))
        {
            return Error::at(original, *span, &d.message).with_code(d.code);
        }
        e
    }
}

pub(super) fn compile(
    source: &str,
    root: &Sexp,
    policy: &BindingPolicy,
    budget: &mut super::generics::Budget,
    depth: usize,
    project_instances: bool,
) -> Result<Component, Error> {
    let fields = root.list().unwrap();
    let mut core_fields = vec![];
    let mut raw = vec![];
    let mut exposes = vec![];
    let mut binds = vec![];
    let mut instances: Vec<InstanceBind> = vec![];
    let mut names = None;
    let mut exports = BTreeSet::new();
    let mut rewritten = false;
    let mut embedded = embedded::Registry::default();
    let mut component_count = 0;
    let mut type_only = true;
    for n in fields
        .iter()
        .skip(1)
        .filter(|n| !n.atom().is_some_and(|s| s.starts_with('$')))
    {
        let p = n.list().unwrap_or(&[]);
        match n.head() {
            Some("@world") => {
                let [_, package, world] = p else {
                    return Err(error(
                        "expected (@world \"namespace:package@version\" \"world\")",
                    ));
                };
                if names.is_some() {
                    return Err(error("duplicate @world"));
                }
                names = Some((
                    package
                        .text()
                        .ok_or_else(|| error("package must be a string"))?
                        .to_owned(),
                    world
                        .text()
                        .ok_or_else(|| error("world must be a string"))?
                        .to_owned(),
                ));
            }
            Some("@lib") => {
                rewritten = true;
                validate_library(source, n)?;
                for d in &p[1..] {
                    type_only &= static_type(d);
                    core_fields.push(d.clone());
                }
            }
            Some("@use" | "@file.use" | "@file.namespace") => {
                rewritten = true;
                type_only &= static_type(n);
                core_fields.push(n.clone());
            }
            Some("@expose") => {
                let [_, name, target] = p else {
                    return Err(error("expected (@expose \"name\" $function<types...>)"));
                };
                let name = name
                    .text()
                    .ok_or_else(|| error("public name must be a string"))?;
                if !exports.insert(name.to_owned()) {
                    return Err(error("duplicate @expose name"));
                }
                let mut export = n.clone();
                if let Kind::List(p) = &mut export.kind {
                    p[0].kind = Kind::Atom("export".into());
                    let mut descriptor = target.clone();
                    let mut head = target.clone();
                    head.kind = Kind::Atom("func".into());
                    descriptor.kind = Kind::List(vec![head, target.clone()]);
                    p[2] = descriptor;
                }
                core_fields.push(export);
                exposes.push(name.to_owned());
            }
            Some("@bind") => {
                rewritten = true;
                super::bindings::declaration(n, &mut binds, &mut instances)?;
            }
            Some("@component.constructor") => {
                let [_, name, payload] = p else {
                    return Err(error("invalid binary constructor declaration"));
                };
                let name = name
                    .atom()
                    .filter(|n| crate::sexpr::identifier(n))
                    .ok_or_else(|| error("constructor requires an identifier"))?;
                let bytes = match &payload.kind {
                    Kind::Text(s) => s.as_bytes().to_vec(),
                    Kind::Bytes(b) => b.clone(),
                    _ => return Err(error("constructor requires binary bytes")),
                };
                embedded::validate(&bytes)?;
                embedded.insert(component_count, bytes);
                component_count += 1;
                raw.push(format!("(component {name})"));
                rewritten = true;
            }
            Some("component") => {
                if crate::syntax::requested(&source[n.span.start..n.span.end]) {
                    let child = super::compile_inner(
                        &source[n.span.start..n.span.end],
                        policy,
                        budget,
                        depth + 1,
                    )
                    .map_err(|mut e| {
                        for label in e
                            .diagnostic
                            .primary
                            .iter_mut()
                            .chain(&mut e.diagnostic.related)
                        {
                            if label.location.path.is_some() {
                                continue;
                            }
                            let span = crate::ir::Span {
                                start: label.span.start + n.span.start,
                                end: label.span.end + n.span.start,
                            };
                            *label = crate::diagnostic::Label::new(source, span, &label.message);
                        }
                        e
                    })?;
                    let name = p
                        .get(1)
                        .and_then(Sexp::atom)
                        .filter(|s| s.starts_with('$'))
                        .unwrap_or("");
                    raw.push(format!("(component {name})"));
                    embedded.insert(component_count, child.wasm);
                    rewritten = true;
                } else {
                    raw.push(source[n.span.start..n.span.end].to_owned());
                }
                component_count += 1;
            }
            Some(head) if head.starts_with('@') && !crate::syntax::annotation(head) => {
                return Err(error(format!("unsupported component declaration {head}")));
            }
            _ => raw.push(source[n.span.start..n.span.end].to_owned()),
        }
    }
    let mut contracts = if !binds.is_empty() || !instances.is_empty() {
        Some(super::bindings::resolve(
            &raw, &embedded, &mut binds, &instances,
        )?)
    } else {
        None
    };
    if exposes.is_empty() {
        if !type_only {
            return Err(error(
                "@lib declarations require @expose in a component entry",
            ));
        }
        if !core_fields.is_empty() {
            let core = implementation(source, &mut core_fields, contracts.as_mut())?;
            crate::check_with_policy(&core.source, policy)
                .map_err(|e| core.diagnostic(source, e))?;
        }
        let text = if names.is_none() && !rewritten {
            source.to_owned()
        } else {
            format!("(component {})", raw.join("\n"))
        };
        let text = if project_instances {
            super::value_exports::instances(text, &embedded)?
        } else {
            text
        };
        return assemble(
            text,
            names.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
            names.is_some(),
            &embedded,
        );
    }
    let core = implementation(source, &mut core_fields, contracts.as_mut())?;
    let checked = crate::check_observed_with(
        &core.source,
        policy,
        |_, _| (),
        |m| super::bindings::apply(m, &binds, &instances, contracts.as_ref()),
    )
    .map_err(|e| core.diagnostic(source, e))?
    .0;
    let built = crate::component::compile_composed(&core.source, checked, contracts.as_ref())
        .map_err(|e| core.diagnostic(source, e))?;
    let mut wiring = String::new();
    let mut required = BTreeSet::new();
    let mut level = 0;
    for p in wasmparser::Parser::new(0).parse_all(&built.wasm) {
        match p.map_err(error)? {
            wasmparser::Payload::Version { .. } => level += 1,
            wasmparser::Payload::End(_) => level -= 1,
            wasmparser::Payload::ComponentImportSection(reader) if level == 1 => {
                for import in reader {
                    required.insert(import.map_err(error)?.name.name.to_owned());
                }
            }
            _ => {}
        }
    }
    let mut groups = BTreeMap::<String, Vec<&Bind>>::new();
    for b in &binds {
        groups.entry(b.module.clone()).or_default().push(b);
    }
    for (module, items) in groups {
        if !required.contains(&module) {
            continue;
        }
        wiring.push_str(&format!(
            " (with {module:?} (instance {}))",
            items
                .iter()
                .map(|b| format!("(export {:?} (func {}))", b.member, b.target))
                .collect::<String>()
        ));
    }
    for b in &instances {
        if !required.contains(&b.module) {
            continue;
        }
        wiring.push_str(&format!(" (with {:?} (instance {}))", b.module, b.target));
    }
    let mut out = String::from("(component\n");
    // Reserve fresh identifiers against *all* source atoms, including nested
    // modules, so macro-looking names cannot capture generated definitions.
    let mut prefix = String::from("$walz-composed");
    while source.contains(&prefix) {
        prefix.push('x');
    }
    for n in &raw {
        out.push_str(n);
        out.push('\n');
    }
    let child = built
        .wat
        .strip_prefix("(component")
        .ok_or_else(|| error("invalid generated component"))?;
    out.push_str(&format!(
        "(component {prefix} {child}\n(instance {prefix}-instance (instantiate {prefix}{wiring}))\n"
    ));
    let mut root_types = false;
    let mut level = 0;
    for p in wasmparser::Parser::new(0).parse_all(&built.wasm) {
        match p.map_err(error)? {
            wasmparser::Payload::Version { .. } => level += 1,
            wasmparser::Payload::End(_) => level -= 1,
            wasmparser::Payload::ComponentExportSection(s) if level == 1 => {
                for e in s {
                    root_types |= e.map_err(error)?.kind == wasmparser::ComponentExternalKind::Type;
                }
            }
            _ => {}
        }
    }
    let mut signatures = BTreeMap::new();
    if root_types {
        let (declarations, functions) = super::value_exports::implementation(
            &out,
            &embedded,
            &format!("{prefix}-instance"),
            &prefix,
        )?;
        out += &declarations;
        signatures = functions;
    }
    for (i, name) in exposes.iter().enumerate() {
        let signature = signatures
            .get(name)
            .map(|s| format!(" (func {s})"))
            .unwrap_or_default();
        out.push_str(&format!("(alias export {prefix}-instance {name:?} (func {prefix}-export{i}))\n(export {name:?} (func {prefix}-export{i}){signature})\n"));
    }
    out.push(')');
    let out = if project_instances {
        super::value_exports::instances(out, &embedded)?
    } else {
        out
    };
    let mut output = assemble(
        out,
        names.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
        depth == 0 || names.is_some(),
        &embedded,
    )?;
    output.manifest["bindings"] = serde_json::to_value(
        binds
            .iter()
            .map(|b| serde_json::json!({"module":b.module,"name":b.member,"target":b.target,"kind":"func"}))
            .chain(instances.iter().map(|b|serde_json::json!({"module":b.module,"target":b.target,"kind":"instance"}))).collect::<Vec<_>>(),
    )
    .unwrap();
    Ok(output)
}

fn implementation(
    source: &str,
    fields: &mut Vec<Sexp>,
    mut contracts: Option<&mut crate::component::bound_imports::BoundImports>,
) -> Result<Text, Error> {
    let types = super::queries::expand(source, fields, contracts.as_deref())?;
    if let Some(c) = contracts.as_mut() {
        c.types.extend(types);
    }
    let mut text = Text::new();
    for n in fields {
        text.push(&render(n), n.span);
    }
    text.source.push(')');
    Ok(text)
}

fn static_type(n: &Sexp) -> bool {
    match n.head() {
        Some("type" | "@type.alias" | "@abi.type" | "@file.namespace") => true,
        Some("@use" | "@file.use") => n
            .list()
            .unwrap()
            .iter()
            .skip(2)
            .all(|n| n.head() == Some("type")),
        _ => false,
    }
}
