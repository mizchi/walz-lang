//! Keep externally named value types attached to the actual instance ports.
//! Unlike resources, these values have structural ABI equality; checked source
//! storage still has one explicit representation per WIT type.
use super::*;
impl World {
    pub(super) fn bind_queried_types(
        &mut self,
        module: &Module,
        types: &BTreeMap<String, WType>,
    ) -> Result<(), Error> {
        for (name, wit) in types {
            if let Some((n, t)) = module
                .types
                .iter()
                .enumerate()
                .find(|(_, t)| &t.name == name)
            {
                let actual = match &t.kind {
                    TypeKind::Struct(_) | TypeKind::Variant(_) => Type::Ref {
                        ty: TypeId(n),
                        nullable: false,
                    },
                    TypeKind::Newtype(_) => Type::Newtype(TypeId(n)),
                    _ => continue,
                };
                self.bind(module, *wit, &actual)?;
            }
        }
        Ok(())
    }
    pub(super) fn discover_value_imports(
        &mut self,
        interfaces: &BTreeMap<String, wit_parser::InterfaceId>,
    ) {
        for (ns, interface) in interfaces {
            for (member, id) in &self.resolve.interfaces[*interface].types {
                let mut id = *id;
                for _ in 0..64 {
                    if let WKind::Type(WType::Id(next)) = self.resolve.types[id].kind {
                        id = next;
                    } else {
                        break;
                    }
                }
                if self.types.contains_key(&id)
                    && matches!(
                        self.resolve.types[id].kind,
                        WKind::Record(_)
                            | WKind::Variant(_)
                            | WKind::Enum(_)
                            | WKind::Flags(_)
                            | WKind::Tuple(_)
                    )
                {
                    self.value_imports
                        .entry(id)
                        .or_insert_with(|| (ns.clone(), member.clone()));
                }
            }
        }
    }
    pub(super) fn alias_inferred_values(&mut self, api: wit_parser::InterfaceId) {
        let aliases = self
            .types
            .iter()
            .filter(|(id, _)| {
                self.resolve.types[**id].owner == wit_parser::TypeOwner::Interface(api)
            })
            .filter_map(|(id, ty)| {
                self.value_imports
                    .keys()
                    .find(|origin| self.types.get(origin) == Some(ty))
                    .map(|origin| (*id, *origin))
            })
            .collect::<Vec<_>>();
        for (id, original) in aliases {
            self.resolve.types[id].kind = WKind::Type(WType::Id(original));
        }
    }
    pub(in crate::component) fn has_native_imports(&self) -> bool {
        self.has_resources() || !self.value_imports.is_empty()
    }
    pub(super) fn uses_bound_value(&self, ty: WType, depth: usize) -> bool {
        if depth > 64 {
            return true;
        }
        let WType::Id(id) = ty else { return false };
        if self.value_imports.contains_key(&id) {
            return true;
        }
        let sub = |t| self.uses_bound_value(t, depth + 1);
        match &self.resolve.types[id].kind {
            WKind::Type(t) | WKind::List(t) | WKind::Option(t) => sub(*t),
            WKind::Tuple(t) => t.types.iter().copied().any(sub),
            WKind::Result(t) => t.ok.into_iter().chain(t.err).any(sub),
            WKind::Future(t) | WKind::Stream(t) => t.is_some_and(sub),
            WKind::Record(t) => t.fields.iter().any(|f| sub(f.ty)),
            WKind::Variant(t) => t.cases.iter().filter_map(|c| c.ty).any(sub),
            _ => false,
        }
    }
    pub(super) fn value_namespaces(&self) -> impl Iterator<Item = &str> {
        self.value_imports.values().map(|(ns, _)| ns.as_str())
    }
    pub(super) fn value_dependencies(
        &self,
        ids: impl IntoIterator<Item = wit_parser::TypeId>,
    ) -> Vec<String> {
        ids.into_iter()
            .filter_map(|id| self.value_imports.get(&id).map(|(ns, _)| ns.clone()))
            .collect()
    }
    pub(super) fn value_aliases(&self, ns: &str, group: usize) -> String {
        self.value_imports
            .iter()
            .filter(|(_, (owner, _))| owner == ns)
            .map(|(id, (_, name))| {
                format!(
                    "(alias export $host{group} {name:?} (type $native_value{}))\n",
                    id.index()
                )
            })
            .collect()
    }
    pub(super) fn values_in(&self, ns: &str) -> Vec<wit_parser::TypeId> {
        self.value_imports
            .iter()
            .filter(|(_, (owner, _))| owner == ns)
            .map(|(id, _)| *id)
            .collect()
    }
    pub(in crate::component) fn native_imports_for(&self, ns: &str) -> bool {
        self.has_native_imports() && self.imported_functions.contains_key(ns)
    }
    pub(in crate::component) fn import_group_index(&self, ns: &str, module: &Module) -> usize {
        let mut names = module
            .functions
            .iter()
            .filter_map(|f| {
                if let FunctionKind::Import { module, .. } = &f.kind {
                    Some(module.as_str())
                } else {
                    None
                }
            })
            .collect::<std::collections::BTreeSet<_>>();
        names.extend(self.value_namespaces());
        names.iter().position(|name| *name == ns).unwrap()
    }
}
