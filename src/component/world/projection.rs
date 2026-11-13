//! Native export ascriptions freshen value naming aliases while preserving
//! resource definitions through explicit, validated instance ports.
use super::*;
pub(crate) fn render(
    resolve: &Resolve,
    interface: wit_parser::InterfaceId,
    instances: &BTreeMap<String, String>,
    prefix: &str,
    root: bool,
) -> Result<(String, String, BTreeMap<String, String>), Error> {
    let mut exports = BTreeMap::new();
    for (name, id) in &resolve.interfaces[interface].types {
        exports
            .entry(canonical(resolve, *id))
            .or_insert_with(|| name.clone());
    }
    let mut outer = String::new();
    let mut declarations = String::new();
    let mut anchors = BTreeMap::new();
    for (id, i) in resolve.interfaces.iter() {
        let Some(namespace) = resolve.id_of(id) else {
            continue;
        };
        let Some(target) = instances.get(&namespace) else {
            continue;
        };
        for (member, ty) in &i.types {
            let id = canonical(resolve, *ty);
            if anchors.contains_key(&id) {
                continue;
            }
            let name = format!("{prefix}-anchor{}", anchors.len());
            outer += &format!("(alias export {target} {member:?} (type {name}))\n");
            let local = if root {
                name.clone()
            } else {
                let local = format!("{name}-local");
                declarations += &format!("(alias outer 1 {name} (type {local}))\n");
                local
            };
            anchors.insert(id, local);
        }
    }
    let this = World {
        resolve: resolve.clone(),
        exports: vec![],
        types: Default::default(),
        names: Default::default(),
        asyncs: Default::default(),
        resources: Default::default(),
        imported_functions: Default::default(),
        inferred_api: None,
        value_imports: Default::default(),
    };
    let mut p = TypeWriter {
        world: &this,
        defined: Default::default(),
        declarations,
        import_namespace: None,
        prefix: format!("{}-projection-", prefix.trim_start_matches('$')),
        public: root,
        projection: Some(exports),
        anchors,
    };
    for (member, id) in &resolve.interfaces[interface].types {
        let id = canonical(resolve, *id);
        let name = p.named(id, 0)?;
        if p.projection.as_ref().unwrap()[&id] != *member {
            if root {
                p.declarations += &format!("(export {member:?} (type {name}))\n");
            } else {
                p.declarations += &format!("(export {member:?} (type (eq {name})))\n");
            }
        }
    }
    let mut functions = BTreeMap::new();
    for (name, f) in &resolve.interfaces[interface].functions {
        let params = f
            .params
            .iter()
            .map(|p0| Ok(format!("(param {:?} {})", p0.name, p.ty(p0.ty, 0)?)))
            .collect::<Result<Vec<_>, Error>>()?
            .join(" ");
        let result = f
            .result
            .map(|t| p.ty(t, 0).map(|t| format!(" (result {t})")))
            .transpose()?
            .unwrap_or_default();
        functions.insert(
            name.clone(),
            format!(
                "{}{params}{result}",
                if f.kind.is_async() { "async " } else { "" }
            ),
        );
    }
    Ok((outer, p.declarations, functions))
}
fn canonical(resolve: &Resolve, mut id: wit_parser::TypeId) -> wit_parser::TypeId {
    for _ in 0..64 {
        let WKind::Type(WType::Id(next)) = resolve.types[id].kind else {
            break;
        };
        id = next;
    }
    id
}
