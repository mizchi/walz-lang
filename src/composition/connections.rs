//! Resolve explicit connections once, then supply contracts before the ordinary
//! type/effect/ownership gate. No host code runs during inference.
use super::*;
use crate::{component::bound_imports::BoundImports, ir::*};
pub(super) struct FunctionConnection {
    pub module: String,
    pub member: String,
    pub target: String,
    pub async_: bool,
    pub params: Vec<String>,
}
pub(super) struct InstanceConnection {
    pub module: String,
    pub target: String,
}
pub(super) fn resolve(
    raw: &[String],
    embedded: &embedded::Registry,
    connections: &mut [FunctionConnection],
    instances: &[InstanceConnection],
) -> Result<BoundImports, Error> {
    let mut probe = String::from("(component ");
    for n in raw {
        // Probe ports are separate from the source's public API, which may also
        // export Core modules or use the same namespace as a selected instance.
        if crate::sexpr::parse(n).is_ok_and(|n| n.head() == Some("export")) {
            continue;
        }
        probe.push_str(n);
    }
    for (i, b) in connections.iter().enumerate() {
        probe.push_str(&format!(
            "(export \"walz-connection-{i}\" (func {}))",
            b.target
        ));
    }
    let mut prefix = String::from("walz-bound-instance-");
    while raw.iter().any(|s| s.contains(&prefix)) {
        prefix.push('x');
    }
    let base = probe.clone();
    for (i, b) in instances.iter().enumerate() {
        probe.push_str(&format!("(export \"{prefix}{i}\" (instance {}))", b.target));
    }
    probe.push(')');
    let bytes = embedded::resolve(&wat::parse_str(&probe).map_err(error)?, embedded)?;
    let checked = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(error)?;
    // Use validator identity to recognize imported types. A WIT projection must
    // reference those definitions instead of re-exporting their alias IDs.
    let imported = imported_types(raw, &checked)?;
    let mut probe = base;
    let mut aliases = vec![];
    let mut export_names = vec![];
    for (i, b) in instances.iter().enumerate() {
        let item = checked
            .component_item_for_export(&format!("{prefix}{i}"))
            .ok_or_else(|| error("missing bound instance"))?;
        let wasmparser::component_types::ComponentEntityType::Instance(id) = item.ty else {
            return Err(error("connection target is not an instance"));
        };
        let mut exports = String::new();
        let mut imported_aliases = vec![];
        for (name, item) in &checked[id].exports {
            let kind = match item.ty {
                wasmparser::component_types::ComponentEntityType::Type { referenced, .. } => {
                    if let Some(origin) = imported.get(&canonical(&checked, referenced)) {
                        imported_aliases.push((name.clone(), origin.clone()));
                        continue;
                    }
                    "type"
                }
                wasmparser::component_types::ComponentEntityType::Func(_) => "func",
                _ => {
                    return Err(error(
                        "a bound interface may export only functions and value/resource types",
                    ));
                }
            };
            exports.push_str(&format!("(export {name:?} ({kind} {} {name:?}))", b.target));
        }
        let export =
            if imported_aliases.is_empty() && b.module.contains(':') && b.module.contains('/') {
                b.module.clone()
            } else {
                format!("{prefix}{i}")
            };
        if imported_aliases.is_empty() {
            // Preserve the instance's own naming IDs. Rebuilding its type
            // exports would leave its function signatures on the old IDs.
            probe.push_str(&format!("(export {export:?} (instance {}))", b.target));
        } else {
            probe.push_str(&format!(
                "(instance ${prefix}{i} {exports})(export {export:?} (instance ${prefix}{i}))"
            ));
        }
        aliases.push(imported_aliases);
        export_names.push(export);
    }
    probe.push(')');
    let bytes = embedded::resolve(&wat::parse_str(&probe).map_err(error)?, embedded)?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(error)?;
    let interface = Interface::decode(&bytes, None)?;
    let mut resolve = interface.resolve.unwrap();
    let world = interface.world.unwrap();
    let mut functions = BTreeMap::<String, BTreeMap<String, wit_parser::Function>>::new();
    let mut interfaces = BTreeMap::new();
    for (i, b) in instances.iter().enumerate() {
        let id = world_interface(&resolve, world, &export_names[i], true)?;
        for (member, (namespace, name)) in &aliases[i] {
            let origin = world_interface(&resolve, world, namespace, false)?;
            let original = *resolve.interfaces[origin]
                .types
                .get(name)
                .ok_or_else(|| error("missing imported type"))?;
            let mut alias = resolve.types[original].clone();
            alias.name = Some(member.clone());
            alias.owner = wit_parser::TypeOwner::Interface(id);
            alias.kind = wit_parser::TypeDefKind::Type(wit_parser::Type::Id(original));
            let alias = resolve.types.alloc(alias);
            resolve.interfaces[id].types.insert(member.clone(), alias);
        }
        interfaces.insert(b.module.clone(), id);
        functions.insert(
            b.module.clone(),
            resolve.interfaces[id]
                .functions
                .iter()
                .map(|(k, f)| (k.clone(), f.clone()))
                .collect(),
        );
    }
    let exports = resolve.worlds[world]
        .exports
        .iter()
        .map(|(k, v)| (resolve.name_world_key(k), v))
        .collect::<BTreeMap<_, _>>();
    for (i, b) in connections.iter_mut().enumerate() {
        let Some(wit_parser::WorldItem::Function(f)) = exports.get(&format!("walz-connection-{i}"))
        else {
            return Err(error("connection target is not a Component function"));
        };
        b.async_ = f.kind.is_async();
        b.params = f.params.iter().map(|p| p.name.clone()).collect();
        functions
            .entry(b.module.clone())
            .or_default()
            .insert(b.member.clone(), (*f).clone());
    }
    Ok(BoundImports {
        resolve,
        interfaces,
        functions,
        types: Default::default(),
    })
}

pub(super) fn apply(
    module: &mut Module,
    connections: &[FunctionConnection],
    instances: &[InstanceConnection],
    contracts: Option<&BoundImports>,
) -> Result<(), Error> {
    let mut pending = vec![];
    for (n, f) in module.functions.iter().enumerate() {
        let FunctionKind::Import { module: ns, name } = &f.kind else {
            pending.push(None);
            continue;
        };
        let mut contract = None;
        if let Some(b) = connections
            .iter()
            .find(|b| &b.module == ns && &b.member == name)
        {
            contract = Some(crate::bindings::Binding {
                params: f
                    .params
                    .iter()
                    .map(|id| module.shape_name(&f.locals[id.0].ty))
                    .collect(),
                result: module.shape_name(&f.result),
                suspends: b.async_,
                capability: format!("component:{ns}#{name}"),
                param_names: b.params.clone(),
                resource: None,
            });
        } else if instances.iter().any(|b| &b.module == ns) {
            let c = contracts.unwrap();
            let (params, result, suspends, param_names, resource) =
                if let Some(f) = c.functions[ns].get(name) {
                    (
                        f.params
                            .iter()
                            .map(|p| crate::wit_bindgen::contract_shape(&c.resolve, p.ty))
                            .collect::<Result<Vec<_>, _>>()?,
                        f.result
                            .map(|t| crate::wit_bindgen::contract_shape(&c.resolve, t))
                            .transpose()?
                            .unwrap_or_else(|| "unit".into()),
                        f.kind.is_async(),
                        f.params.iter().map(|p| p.name.clone()).collect(),
                        None,
                    )
                } else if let Some(resource_name) = name.strip_prefix("[resource-drop]") {
                    let interface = &c.resolve.interfaces[c.interfaces[ns]];
                    let id = *interface.types.get(resource_name).ok_or_else(|| {
                        error(format!("missing resource export {ns}#{resource_name}"))
                    })?;
                    let id = crate::wit_bindgen::canonical_resource(&c.resolve, id)?;
                    let kind = crate::wit_bindgen::resource_kind(&c.resolve, id)?;
                    if !module.types.iter().any(
                    |t| matches!(&t.kind,TypeKind::Resource{kind:k,close} if *k==kind&&close.0==n),
                ) {
                    return Err(error(
                        "a canonical drop port must be the declared resource destructor",
                    ));
                }
                    (
                        vec!["i32".into()],
                        "unit".into(),
                        false,
                        vec![],
                        Some(crate::bindings::ResourceBinding {
                            kind,
                            operation: crate::bindings::ResourceOperation::Close,
                        }),
                    )
                } else {
                    return Err(error(format!("missing Component function {ns}#{name}")));
                };
            contract = Some(crate::bindings::Binding {
                params,
                result,
                suspends,
                param_names,
                resource,
                capability: format!("component:{ns}#{name}"),
            });
        }
        pending.push(contract);
    }
    for (f, c) in module.functions.iter_mut().zip(pending) {
        if c.is_some() {
            f.import_contract = c;
        }
    }
    Ok(())
}

fn world_interface(
    r: &wit_parser::Resolve,
    world: wit_parser::WorldId,
    name: &str,
    export: bool,
) -> Result<wit_parser::InterfaceId, Error> {
    let ports = if export {
        &r.worlds[world].exports
    } else {
        &r.worlds[world].imports
    };
    ports
        .iter()
        .find_map(|(k, v)| {
            if r.name_world_key(k) == name {
                if let wit_parser::WorldItem::Interface { id, .. } = v {
                    Some(*id)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or_else(|| error(format!("missing interface {name}")))
}
#[derive(Eq, PartialEq, Hash)]
enum CanonicalType {
    Resource(wasmparser::component_types::ResourceId),
    Other(wasmparser::component_types::ComponentAnyTypeId),
}
fn canonical(
    types: &wasmparser::types::Types,
    mut id: wasmparser::component_types::ComponentAnyTypeId,
) -> CanonicalType {
    while let Some(next) = types.peel_alias(id) {
        id = next;
    }
    if let wasmparser::component_types::ComponentAnyTypeId::Resource(r) = id {
        CanonicalType::Resource(r.resource())
    } else {
        CanonicalType::Other(id)
    }
}
fn imported_types(
    raw: &[String],
    types: &wasmparser::types::Types,
) -> Result<std::collections::HashMap<CanonicalType, (String, String)>, Error> {
    let mut imported = std::collections::HashMap::new();
    for source in raw {
        let n = crate::sexpr::parse(source)?;
        if n.head() != Some("import") {
            continue;
        }
        let Some(name) = n.list().and_then(|p| p.get(1)).and_then(Sexp::text) else {
            continue;
        };
        let Some(item) = types.component_item_for_import(name) else {
            continue;
        };
        let wasmparser::component_types::ComponentEntityType::Instance(id) = item.ty else {
            continue;
        };
        for (member, item) in &types[id].exports {
            if let wasmparser::component_types::ComponentEntityType::Type { referenced, .. } =
                item.ty
            {
                imported
                    .entry(canonical(types, referenced))
                    .or_insert_with(|| (name.into(), member.clone()));
            }
        }
    }
    Ok(imported)
}

// Shared by staged type inference and the final implementation wiring.
pub(super) fn declaration(
    n: &Sexp,
    connections: &mut Vec<FunctionConnection>,
    instances: &mut Vec<InstanceConnection>,
) -> Result<(), Error> {
    let p = n.list().unwrap_or(&[]);
    if let [_, module, target] = p {
        let module = module
            .text()
            .ok_or_else(|| error("connection module must be a string"))?
            .to_owned();
        let t = target
            .list()
            .filter(|p| p.len() == 2 && p[0].atom() == Some("instance"))
            .and_then(|p| p[1].atom())
            .filter(|n| crate::sexpr::identifier(n))
            .ok_or_else(|| error("expected (@connect \"module\" (instance $target))"))?
            .to_owned();
        if instances.iter().any(|b| b.module == module)
            || connections
                .iter()
                .any(|b: &FunctionConnection| b.module == module)
        {
            return Err(error("duplicate @connect namespace"));
        }
        fixed(&module)?;
        instances.push(InstanceConnection { module, target: t });
        return Ok(());
    }
    let [_, module, member, target] = p else {
        return Err(error(
            "expected (@connect \"module\" \"member\" $component-function)",
        ));
    };
    let module = module
        .text()
        .ok_or_else(|| error("connection module must be a string"))?
        .to_owned();
    let member = member
        .text()
        .ok_or_else(|| error("connection member must be a string"))?
        .to_owned();
    let target = target
        .atom()
        .filter(|n| n.starts_with('$'))
        .ok_or_else(|| error("connection target must name a Component function"))?
        .to_owned();
    if instances.iter().any(|b| b.module == module) {
        return Err(error("duplicate @connect namespace"));
    }
    if connections
        .iter()
        .any(|b: &FunctionConnection| b.module == module && b.member == member)
    {
        return Err(error("duplicate @connect port"));
    }
    let (async_, params) = (false, vec![]);
    if matches!(
        module.as_str(),
        crate::bindings::EVENT_LOOP | crate::bindings::TEXT_IO | crate::bindings::CLI
    ) {
        return Err(error("@connect cannot override fixed host contracts"));
    }
    connections.push(FunctionConnection {
        module,
        member,
        target,
        async_,
        params,
    });
    Ok(())
}

fn fixed(module: &str) -> Result<(), Error> {
    if matches!(
        module,
        crate::bindings::EVENT_LOOP | crate::bindings::TEXT_IO | crate::bindings::CLI
    ) {
        return Err(error("@connect cannot override fixed host contracts"));
    }
    Ok(())
}
