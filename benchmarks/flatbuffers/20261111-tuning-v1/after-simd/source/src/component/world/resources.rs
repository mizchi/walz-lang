//! Imported WIT resource identity and typed own/borrow bindings.
use super::*;
use crate::bindings::ResourceOperation;
#[derive(Clone)]
pub(super) struct Resource {
    pub wit: wit_parser::TypeId,
    pub namespace: String,
    pub name: String,
}
impl World {
    pub(in crate::component) fn discover_resources(
        &mut self,
        module: &Module,
        world: &wit_parser::World,
    ) -> Result<(), Error> {
        for (n, t) in module.types.iter().enumerate() {
            let TypeKind::Resource { kind, .. } = &t.kind else {
                continue;
            };
            let mut found = None;
            for item in world.imports.values() {
                let WorldItem::Interface { id, .. } = item else {
                    continue;
                };
                let ns = self
                    .resolve
                    .id_of(*id)
                    .ok_or_else(|| fail("anonymous resource interface"))?;
                for (name, id) in &self.resolve.interfaces[*id].types {
                    if let Ok(resource) = crate::wit_bindgen::canonical_resource(&self.resolve, *id)
                        && *kind == crate::wit_bindgen::resource_kind(&self.resolve, resource)?
                    {
                        let candidate = Resource {
                            wit: resource,
                            namespace: ns.clone(),
                            name: name.clone(),
                        };
                        let direct = matches!(self.resolve.types[resource].owner,wit_parser::TypeOwner::Interface(owner) if self.resolve.id_of(owner).as_deref()==Some(ns.as_str()));
                        if found.is_none() || direct {
                            found = Some(candidate);
                        }
                    }
                }
            }
            self.resources.insert(
                TypeId(n),
                found.ok_or_else(|| {
                    fail(format!(
                        "resource {kind} must name an imported WIT resource"
                    ))
                })?,
            );
        }
        Ok(())
    }
    pub(in crate::component) fn resource_id(&self, t: WType) -> Option<(wit_parser::TypeId, bool)> {
        let mut t = t;
        for _ in 0..64 {
            let WType::Id(id) = t else { return None };
            match self.resolve.types[id].kind {
                WKind::Handle(wit_parser::Handle::Own(r)) => {
                    return self.resource_id(WType::Id(r)).map(|(id, _)| (id, false));
                }
                WKind::Handle(wit_parser::Handle::Borrow(r)) => {
                    return self.resource_id(WType::Id(r)).map(|(id, _)| (id, true));
                }
                WKind::Resource => return Some((id, false)),
                WKind::Type(other) => t = other,
                _ => return None,
            }
        }
        None
    }
    pub(in crate::component) fn bind_resource(
        &mut self,
        t: WType,
        actual: &Type,
    ) -> Result<bool, Error> {
        let Some((r, borrow)) = self.resource_id(t) else {
            return Ok(false);
        };
        let id = match actual {
            Type::Resource(id) if !borrow => id,
            Type::Borrow(id) if borrow => id,
            _ => {
                return Err(fail(
                    "WIT own/borrow requires the corresponding affine resource type",
                ));
            }
        };
        if self.resources.get(id).is_none_or(|x| x.wit != r) {
            return Err(fail(
                "WIT resource identity differs from the source resource kind",
            ));
        }
        if let Some(old) = self.types.insert(r, Type::Resource(*id))
            && old != Type::Resource(*id)
        {
            return Err(fail("one WIT resource maps to different source types"));
        }
        if let WType::Id(t) = t
            && t != r
        {
            self.types.insert(t, actual.clone());
        }
        Ok(true)
    }
    pub(in crate::component) fn close_resource(
        &self,
        module: &Module,
        f: &Function,
    ) -> Option<TypeId> {
        self.resources.keys().find(|id|matches!(&module.types[id.0].kind,TypeKind::Resource{close,..} if module.functions[close.0].name==f.name)).copied()
    }
    pub(in crate::component) fn check_close(
        &self,
        module: &Module,
        f: &Function,
    ) -> Result<bool, Error> {
        let Some(id) = self.close_resource(module, f) else {
            return Ok(false);
        };
        let r = &self.resources[&id];
        if !matches!(&f.kind,FunctionKind::Import{module,name} if *module==r.namespace && *name==format!("[resource-drop]{}",r.name))
        {
            return Err(fail(
                "native resource destructor must use its WIT [resource-drop] binding",
            ));
        }
        Ok(true)
    }
    pub(in crate::component) fn has_resources(&self) -> bool {
        !self.resources.is_empty()
    }
    pub(in crate::component) fn resource_arguments(&self) -> String {
        self.resources
            .keys()
            .map(|id| {
                format!(
                    " (with \"resource{}\" (type $native_resource{}))",
                    id.0, id.0
                )
            })
            .collect()
    }
    pub(in crate::component) fn import_prefix(&self, module: &Module) -> Result<String, Error> {
        let mut groups = BTreeMap::<String, Vec<&Function>>::new();
        for f in &module.functions {
            if let FunctionKind::Import { module, .. } = &f.kind {
                groups.entry(module.clone()).or_default().push(f);
            }
        }
        let indices = groups
            .keys()
            .enumerate()
            .map(|(n, k)| (k.clone(), n))
            .collect::<BTreeMap<_, _>>();
        let mut out = String::new();
        let mut done = std::collections::BTreeSet::new();
        let mut visiting = std::collections::BTreeSet::new();
        for ns in groups.keys() {
            self.import_group(ns, &groups, &indices, &mut done, &mut visiting, &mut out)?;
        }
        Ok(out)
    }
    fn import_group(
        &self,
        ns: &str,
        groups: &BTreeMap<String, Vec<&Function>>,
        indices: &BTreeMap<String, usize>,
        done: &mut std::collections::BTreeSet<String>,
        visiting: &mut std::collections::BTreeSet<String>,
        out: &mut String,
    ) -> Result<(), Error> {
        if done.contains(ns) {
            return Ok(());
        }
        if !visiting.insert(ns.into()) {
            return Err(fail("cyclic resource interface dependencies"));
        }
        let interface = self
            .resolve
            .interfaces
            .iter()
            .find(|(id, _)| self.resolve.id_of(*id).as_deref() == Some(ns))
            .map(|(_, i)| i)
            .ok_or_else(|| fail(format!("missing WIT interface {ns}")))?;
        let group = indices[ns];
        let mut printer = TypeWriter {
            world: self,
            defined: BTreeMap::new(),
            declarations: String::new(),
            import_namespace: Some(ns.into()),
            prefix: format!("native_host{group}_"),
        };
        let mut funcs = String::new();
        for f in &groups[ns] {
            let FunctionKind::Import { name, .. } = &f.kind else {
                unreachable!()
            };
            if f.import_contract
                .as_ref()
                .and_then(|b| b.resource.as_ref())
                .is_some_and(|r| r.operation == ResourceOperation::Close)
            {
                continue;
            }
            let expected = interface
                .functions
                .get(name)
                .ok_or_else(|| fail(format!("missing WIT host function {ns}#{name}")))?;
            let params = expected
                .params
                .iter()
                .map(|p| Ok(format!("(param {:?} {})", p.name, printer.ty(p.ty, 0)?)))
                .collect::<Result<Vec<_>, Error>>()?
                .join(" ");
            let result = expected
                .result
                .map(|t| printer.ty(t, 0).map(|t| format!(" (result {t})")))
                .transpose()?
                .unwrap_or_default();
            funcs += &format!(
                "(export {name:?} (func {}{params}{result}))\n",
                if expected.kind.is_async() {
                    "async "
                } else {
                    ""
                }
            );
        }
        for r in self.resources.values().filter(|r| r.namespace == ns) {
            printer.named(r.wit, 0)?;
        }
        for resource in printer
            .defined
            .keys()
            .filter_map(|id| self.resources.values().find(|r| r.wit == *id))
        {
            if resource.namespace != ns {
                self.import_group(&resource.namespace, groups, indices, done, visiting, out)?;
            }
        }
        out.push_str(&format!(
            "(import {ns:?} (instance $host{group} {}{funcs}))\n",
            printer.declarations
        ));
        for (id, r) in self.resources.iter().filter(|(_, r)| r.namespace == ns) {
            out.push_str(&format!(
                "(alias export $host{group} {:?} (type $native_resource{}))\n",
                r.name, id.0
            ));
        }
        visiting.remove(ns);
        done.insert(ns.into());
        Ok(())
    }
}
