//! Named shared values require fresh naming aliases at each public interface.
//! Export ascriptions change only the visible type; native subtyping verifies
//! compatibility and resource aliases still point at actual instance ports.
use super::*;
use wasmparser::component_types::{
    ComponentAnyTypeId as Id, ComponentDefinedType as Defined, ComponentEntityType as Entity,
};

fn imports(raw: &[String]) -> BTreeMap<String, String> {
    raw.iter()
        .filter_map(|text| {
            let n = crate::sexpr::parse(text).ok()?;
            let p = n.list()?;
            if n.head() != Some("import") || p.get(2)?.head() != Some("instance") {
                return None;
            }
            Some((
                p[1].text()?.to_owned(),
                p[2].list()?.get(1)?.atom()?.to_owned(),
            ))
        })
        .collect()
}
fn project(
    raw: &[String],
    embedded: &embedded::Registry,
    target: &str,
    prefix: &str,
    root: bool,
) -> Result<(String, String, BTreeMap<String, String>), Error> {
    let ns = "walz-value-projection";
    let bound = connections::resolve(
        raw,
        embedded,
        &mut [],
        &[connections::InstanceConnection {
            module: ns.into(),
            target: target.into(),
        }],
    )?;
    crate::component::project_interface(
        &bound.resolve,
        bound.interfaces[ns],
        &imports(raw),
        prefix,
        root,
    )
}
pub(super) fn implementation(
    text: &str,
    embedded: &embedded::Registry,
    target: &str,
    prefix: &str,
) -> Result<(String, BTreeMap<String, String>), Error> {
    let root = crate::sexpr::parse(&format!("{text})"))?;
    let raw = root.list().unwrap()[1..]
        .iter()
        .map(render)
        .collect::<Vec<_>>();
    let (outer, declarations, signatures) = project(&raw, embedded, target, prefix, true)?;
    Ok((format!("{outer}{declarations}"), signatures))
}
pub(super) fn instances(text: String, embedded: &embedded::Registry) -> Result<String, Error> {
    let root = crate::sexpr::parse(&text)?;
    let raw = root.list().unwrap()[1..]
        .iter()
        .map(render)
        .collect::<Vec<_>>();
    let bytes = embedded::resolve(&wat::parse_str(&text).map_err(error)?, embedded)?;
    let types = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(error)?;
    let mut prefix = "$walz-value-export".to_owned();
    while text.contains(&prefix) {
        prefix.push('x');
    }
    let mut output = String::from("(component ");
    let mut count = 0;
    for (node, original) in root.list().unwrap()[1..].iter().zip(&raw) {
        let p = node.list().unwrap_or(&[]);
        let name = p.get(1).and_then(Sexp::text);
        let target = p
            .get(2)
            .filter(|n| n.head() == Some("instance"))
            .and_then(Sexp::list)
            .and_then(|p| p.get(1))
            .and_then(Sexp::atom);
        let shape = name.and_then(|name| types.component_item_for_export(name));
        let instance = shape.and_then(|i| {
            if let Entity::Instance(id) = i.ty {
                Some(id)
            } else {
                None
            }
        });
        let shared_values = instance.is_some_and(|id| {
            types[id].exports.values().any(|v| {
                if let Entity::Type {
                    referenced: Id::Defined(id),
                    ..
                } = v.ty
                {
                    matches!(
                        types[id],
                        Defined::Record(_)
                            | Defined::Variant(_)
                            | Defined::Enum(_)
                            | Defined::Flags(_)
                    )
                } else {
                    false
                }
            })
        });
        let flat = instance.is_some_and(|id| {
            types[id]
                .exports
                .values()
                .all(|v| matches!(v.ty, Entity::Type { .. } | Entity::Func(_)))
        });
        if node.head() != Some("export")
            || p.len() != 3
            || !shared_values
            || !flat
            || target.is_none()
        {
            output.push_str(original);
            output.push('\n');
            continue;
        }
        count += 1;
        if count > 1024 {
            return Err(error("value export ascription count limit exceeded"));
        }
        let fresh = format!("{prefix}{count}");
        let (outer, mut declarations, functions) =
            project(&raw, embedded, target.unwrap(), &fresh, false)?;
        for (member, signature) in functions {
            declarations += &format!("(export {member:?} (func {signature}))\n");
        }
        output += &format!(
            "{outer}(type {fresh} (instance {declarations}))\n(export {:?} (instance {}) (instance (type {fresh})))\n",
            name.unwrap(),
            target.unwrap()
        );
        if output.len() > 16 * 1024 * 1024 {
            return Err(error("value export ascription size limit exceeded"));
        }
    }
    output.push(')');
    Ok(output)
}
