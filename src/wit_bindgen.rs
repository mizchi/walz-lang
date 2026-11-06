//! WIT supplies shapes. A separate, host-owned policy supplies authority.
use crate::{
    Error,
    bindings::{Binding, BindingPolicy, PolicyImport},
};
use std::path::Path;
use wit_parser::{FunctionKind, Resolve, Type, TypeDefKind, WorldItem};

#[derive(Debug)]
pub struct Generated {
    pub source: String,
    pub policy: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Grants {
    imports: Vec<Grant>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    module: String,
    name: String,
    suspends: bool,
    capability: String,
}

pub fn generate(path: &Path, world: Option<&str>, grants: &str) -> Result<Generated, Error> {
    let fail = |e| Error::message(format!("WIT binding: {e}"));
    let grants: Grants = serde_json::from_str(grants).map_err(|e| fail(e.to_string()))?;
    let mut seen = std::collections::BTreeSet::new();
    for g in &grants.imports {
        if g.capability.is_empty() || !seen.insert((&g.module, &g.name)) {
            return Err(fail(
                "policy requires unique imports and nonempty capabilities".into(),
            ));
        }
    }
    let mut resolve = Resolve::default();
    let (package, _) = resolve.push_path(path).map_err(|e| fail(e.to_string()))?;
    let world = resolve
        .select_world(&[package], world)
        .map_err(|e| fail(e.to_string()))?;
    let mut source =
        String::from(";; Generated from WIT. Trust comes from bindings.json.\n(module\n");
    let mut policy = BindingPolicy::default();
    let mut names = std::collections::BTreeSet::new();
    let mut standard_types = std::collections::BTreeSet::new();
    for item in resolve.worlds[world].imports.values() {
        let WorldItem::Interface { id, .. } = item else {
            return Err(fail(
                "unsupported WIT world import: expected an interface".into(),
            ));
        };
        let interface = &resolve.interfaces[*id];
        let namespace = resolve
            .id_of(*id)
            .ok_or_else(|| fail("anonymous interfaces are unsupported".into()))?;
        for function in interface.functions.values() {
            let grant = grants
                .imports
                .iter()
                .find(|p| p.module == namespace && p.name == function.name)
                .ok_or_else(|| fail(format!("missing policy for {namespace}#{}", function.name)))?;
            let async_ = match function.kind {
                FunctionKind::Freestanding => false,
                FunctionKind::AsyncFreestanding => true,
                _ => return Err(fail("unsupported WIT resource method".into())),
            };
            if async_ != grant.suspends {
                return Err(fail(format!(
                    "policy suspension disagrees with WIT for {}",
                    function.name
                )));
            }
            let params = function
                .params
                .iter()
                .map(|p| shape(&resolve, p.ty, 0, ""))
                .collect::<Result<Vec<_>, _>>()?;
            let result = function
                .result
                .map(|t| shape(&resolve, t, 0, ""))
                .transpose()?
                .unwrap_or("unit".into());
            let local = format!("${}.{}", interface.name.as_deref().unwrap(), function.name);
            if !names.insert(local.clone()) {
                return Err(fail(format!("duplicate local name {local}")));
            }
            source.push_str(&format!(
                "  (import {namespace:?} {:?} (func {local}",
                function.name
            ));
            for param in &function.params {
                let ty = shape(&resolve, param.ty, 0, "$")?;
                standard_types.extend(
                    ty.split(['(', ')', ' '])
                        .filter(|t| t.starts_with('$'))
                        .map(str::to_owned),
                );
                source.push_str(&format!(" (param ${} {ty})", param.name));
            }
            if let Some(result) = function.result {
                let ty = shape(&resolve, result, 0, "$")?;
                standard_types.extend(
                    ty.split(['(', ')', ' '])
                        .filter(|t| t.starts_with('$'))
                        .map(str::to_owned),
                );
                source.push_str(&format!(" (result {ty})"));
            }
            source.push_str(&format!("))\n  (%export (func {local}))\n"));
            policy.imports.push(PolicyImport {
                module: namespace.clone(),
                name: function.name.clone(),
                binding: Binding {
                    params,
                    result,
                    suspends: grant.suspends,
                    capability: grant.capability.clone(),
                    resource: None,
                    param_names: function.params.iter().map(|p| p.name.clone()).collect(),
                },
            });
        }
    }
    if policy.imports.len() != grants.imports.len() {
        return Err(fail(
            "policy contains imports outside the selected world".into(),
        ));
    }
    source.push_str(")\n");
    if !standard_types.is_empty() {
        let imports = standard_types
            .iter()
            .map(|name| format!(" (type {name})"))
            .collect::<String>();
        let start = source.find("(module\n").unwrap() + "(module\n".len();
        source.insert_str(start, &format!("  (%use \"wlt:core\"{imports})\n"));
    }
    let policy = serde_json::to_string_pretty(&policy).unwrap();
    BindingPolicy::parse(&policy)?;
    Ok(Generated { source, policy })
}

fn shape(r: &Resolve, ty: Type, depth: usize, prefix: &str) -> Result<String, Error> {
    if depth > 64 {
        return Err(Error::message("WIT type nesting exceeds 64".into()));
    }
    let sub = |t| shape(r, t, depth + 1, prefix);
    let constructor = |name: &str| {
        if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("%inst ${name}")
        }
    };
    let optional = |t: Option<Type>| {
        t.map(sub)
            .transpose()
            .map(|x| x.unwrap_or_else(|| format!("{prefix}unit")))
    };
    Ok(match ty {
        Type::S32 => "i32".into(),
        Type::S64 => "i64".into(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        // Source aliases are independent of the policy's stable `text` shape.
        Type::String if prefix == "$" => "$string".into(),
        Type::String => format!("{prefix}text"),
        Type::Id(id) => match &r.types[id].kind {
            TypeDefKind::Type(t) => sub(*t)?,
            TypeDefKind::List(Type::U8) => format!("{prefix}bytes"),
            TypeDefKind::List(t) => {
                let payload = sub(*t)?;
                if contains_endpoint(r, *t, 0) {
                    return Err(Error::message("WIT list elements must be shared data; future/stream elements are unsupported".into()));
                }
                format!("({} {payload})", constructor("list"))
            }
            TypeDefKind::Option(t) => format!("({} {})", constructor("option"), sub(*t)?),
            TypeDefKind::Result(t) => {
                format!(
                    "({} {} {})",
                    constructor("result"),
                    optional(t.ok)?,
                    optional(t.err)?
                )
            }
            TypeDefKind::Tuple(t) if t.types.len() == 2 => {
                format!(
                    "({} {} {})",
                    constructor(if prefix.is_empty() { "pair" } else { "tuple" }),
                    sub(t.types[0])?,
                    sub(t.types[1])?
                )
            }
            TypeDefKind::Future(t) => format!("({} {})", constructor("future"), optional(*t)?),
            TypeDefKind::Stream(t) => format!("({} {})", constructor("stream"), optional(*t)?),
            t => {
                return Err(Error::message(format!(
                    "unsupported WIT type: {}",
                    t.as_str()
                )));
            }
        },
        t => return Err(Error::message(format!("unsupported WIT primitive: {t:?}"))),
    })
}

fn contains_endpoint(r: &Resolve, ty: Type, depth: usize) -> bool {
    if depth > 64 {
        return true;
    }
    let Type::Id(id) = ty else { return false };
    let sub = |t| contains_endpoint(r, t, depth + 1);
    match &r.types[id].kind {
        TypeDefKind::Future(_) | TypeDefKind::Stream(_) => true,
        TypeDefKind::Type(t) | TypeDefKind::List(t) | TypeDefKind::Option(t) => sub(*t),
        TypeDefKind::Tuple(t) => t.types.iter().copied().any(sub),
        TypeDefKind::Result(t) => t.ok.into_iter().chain(t.err).any(sub),
        _ => false,
    }
}
