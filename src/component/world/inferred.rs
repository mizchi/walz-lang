//! Inferred value signatures alias resource definitions from a single decoded
//! connection graph. They never materialize a fresh nominal resource.
use super::*;
use crate::component::bound_imports::BoundImports;
pub(super) fn prepare(
    module: &Module,
    imports: Option<&BoundImports>,
) -> Result<(Resolve, wit_parser::InterfaceId), Error> {
    let mut resolve = imports.map(|b| b.resolve.clone()).unwrap_or_default();
    let mut suffix = 0;
    let package = loop {
        let name = format!("composition-inferred-a{suffix}");
        if resolve
            .packages
            .iter()
            .all(|(_, p)| p.name.namespace != "walz" || p.name.name != name)
        {
            break format!("walz:{name}");
        }
        suffix += 1;
    };
    let text = crate::wit::emit_functions(module)?.replacen(
        "package walz:generated@0.1.0;",
        &format!("package {package};"),
        1,
    );
    let package = resolve
        .push_str("inferred.wit", &text)
        .map_err(|e| fail(e.to_string()))?;
    let api = *resolve.packages[package]
        .interfaces
        .get("api")
        .ok_or_else(|| fail("missing inferred interface"))?;
    Ok((resolve, api))
}
impl World {
    pub(super) fn bind_inferred_resources(
        &mut self,
        api: wit_parser::InterfaceId,
    ) -> Result<(), Error> {
        let mut aliases = BTreeMap::new();
        for (source, r) in &self.resources {
            if self
                .types
                .insert(r.wit, Type::Resource(*source))
                .is_some_and(|t| t != Type::Resource(*source))
            {
                return Err(fail(
                    "one WIT resource cannot map to distinct source resource types",
                ));
            }
            if let Some(alias) = self.resolve.interfaces[api]
                .types
                .get(&format!("resource-{}", source.0))
            {
                self.resolve.types[*alias].kind = WKind::Type(WType::Id(r.wit));
                aliases.insert(*alias, r.wit);
            }
        }
        for (_, t) in self.resolve.types.iter_mut() {
            if let WKind::Handle(wit_parser::Handle::Own(r) | wit_parser::Handle::Borrow(r)) =
                &mut t.kind
                && let Some(actual) = aliases.get(r)
            {
                *r = *actual;
            }
        }
        Ok(())
    }
    pub(super) fn check_inferred_imports(&mut self, module: &Module) -> Result<(), Error> {
        for f in &module.functions {
            let FunctionKind::Import {
                module: namespace,
                name,
            } = &f.kind
            else {
                continue;
            };
            if self.check_close(module, f)? {
                continue;
            }
            if let Some(expected) = self
                .imported_functions
                .get(namespace)
                .and_then(|f| f.get(name))
                .cloned()
            {
                self.signature(module, f, &expected)?;
                self.names.insert(
                    format!("{namespace}#{name}"),
                    expected.params.iter().map(|p| p.name.clone()).collect(),
                );
            } else if self.has_resources() {
                return Err(fail(format!(
                    "resource composition requires an explicit binding for {namespace}#{name}"
                )));
            }
        }
        Ok(())
    }
}

pub(super) fn uses_resource(r: &Resolve, t: WType, depth: usize) -> bool {
    if depth > 64 {
        return true;
    }
    let WType::Id(id) = t else {
        return false;
    };
    let sub = |t| uses_resource(r, t, depth + 1);
    match &r.types[id].kind {
        WKind::Resource | WKind::Handle(_) => true,
        WKind::Type(t) | WKind::Option(t) | WKind::List(t) => sub(*t),
        WKind::Tuple(t) => t.types.iter().copied().any(sub),
        WKind::Result(t) => t.ok.into_iter().chain(t.err).any(sub),
        WKind::Future(t) | WKind::Stream(t) => t.is_some_and(sub),
        WKind::Record(t) => t.fields.iter().any(|f| sub(f.ty)),
        WKind::Variant(t) => t.cases.iter().filter_map(|c| c.ty).any(sub),
        _ => false,
    }
}
impl World {
    pub(in crate::component) fn public_signatures(
        &self,
    ) -> Result<(String, BTreeMap<String, String>), Error> {
        let mut signatures = BTreeMap::new();
        let Some(api) = self.inferred_api else {
            return Ok((String::new(), signatures));
        };
        let mut printer = TypeWriter {
            projection: None,
            anchors: Default::default(),
            world: self,
            defined: Default::default(),
            declarations: String::new(),
            import_namespace: None,
            prefix: "public_".into(),
            public: true,
        };
        for (name, f) in &self.resolve.interfaces[api].functions {
            let params = f
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    Ok(format!(
                        "(param {:?} {})",
                        self.names(name).and_then(|n| n.get(i)).unwrap_or(&p.name),
                        printer.ty(p.ty, 0)?
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?
                .join(" ");
            let result = f
                .result
                .map(|t| printer.ty(t, 0).map(|t| format!(" (result {t})")))
                .transpose()?
                .unwrap_or_default();
            signatures.insert(
                name.clone(),
                format!(
                    "{}{params}{result}",
                    if f.kind.is_async() { "async " } else { "" }
                ),
            );
        }
        Ok((printer.declarations, signatures))
    }
}
