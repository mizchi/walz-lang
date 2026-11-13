//! One resolved public interface graph supplies WIT and the build manifest.
use super::*;
#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub name: String,
    pub kind: String,
    pub asynchronous: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Interface {
    pub imports: Vec<Item>,
    pub exports: Vec<Item>,
    pub wit: String,
    pub unsupported: Option<String>,
    #[serde(skip)]
    pub(crate) resolve: Option<wit_parser::Resolve>,
    #[serde(skip)]
    pub(crate) world: Option<wit_parser::WorldId>,
}
impl Interface {
    /// Assert the selected external contract, comparing value shapes and
    /// nominal resource identity before publishing outputs.
    pub fn validate_world(&self, path: &std::path::Path, name: Option<&str>) -> Result<(), Error> {
        let fail = |e: String| Error::message(e).with_code("E_WIT_WORLD");
        if self.resolve.is_none() {
            return Err(fail("this Component has no WIT representation".into()));
        }
        let mut actual = wit_parser::Resolve::default();
        let package = actual
            .push_str("actual.wit", &self.wit)
            .map_err(|e| fail(e.to_string()))?;
        let actual_world = actual
            .select_world(&[package], None)
            .map_err(|e| fail(e.to_string()))?;
        let mut expected = wit_parser::Resolve::default();
        let (pkg, _) = expected.push_path(path).map_err(|e| fail(e.to_string()))?;
        let world = expected
            .select_world(&[pkg], name)
            .map_err(|e| fail(e.to_string()))?;
        super::contract::validate(&actual, actual_world, &expected, world)
    }
    pub(super) fn opaque(message: String) -> Self {
        Self {
            imports: vec![],
            exports: vec![],
            wit: String::new(),
            unsupported: Some(message),
            resolve: None,
            world: None,
        }
    }

    pub(super) fn decode(bytes: &[u8], names: Option<(&str, &str)>) -> Result<Self, Error> {
        super::wit_projection::validate(bytes)?;
        let recorded = validate_metadata(bytes)?;
        let names = names.or_else(|| recorded.as_ref().map(|(a, b)| (a.as_str(), b.as_str())));
        let wit_parser::decoding::DecodedWasm::Component(mut resolve, world) =
            wit_parser::decoding::decode(bytes).map_err(|e| error(format!("{e:#}")))?
        else {
            return Err(error("expected executable component"));
        };
        let pkg = resolve.worlds[world]
            .package
            .ok_or_else(|| error("missing component package"))?;
        if let Some((package, name)) = names {
            let mut declared = wit_parser::Resolve::default();
            let id = declared
                .push_str(
                    "world.wit",
                    &format!("package {package}; world {name} {{}}"),
                )
                .map_err(error)?;
            if declared.packages[id].name.to_string() != package
                || declared.packages[id].worlds.len() != 1
                || !declared.packages[id].worlds.contains_key(name)
                || !declared.packages[id].interfaces.is_empty()
            {
                return Err(error("invalid package or world name"));
            }
            let old_package = resolve.packages[pkg].name.clone();
            let new_package = declared.packages[id].name.clone();
            if resolve
                .package_names
                .get(&new_package)
                .is_some_and(|id| *id != pkg)
            {
                return Err(error(
                    "world package name conflicts with an imported package",
                ));
            }
            resolve.package_names.shift_remove(&old_package);
            resolve.package_names.insert(new_package.clone(), pkg);
            resolve.packages[pkg].name = new_package;
            let old = resolve.worlds[world].name.clone();
            resolve.packages[pkg].worlds.shift_remove(&old);
            resolve.packages[pkg].worlds.insert(name.into(), world);
            resolve.worlds[world].name = name.into();
        }
        let imports = resolve.worlds[world]
            .imports
            .iter()
            .map(|(k, v)| Item {
                name: resolve.name_world_key(k),
                kind: kind(v).into(),
                asynchronous: matches!(v,wit_parser::WorldItem::Function(f) if f.kind.is_async()),
            })
            .collect();
        let exports = resolve.worlds[world]
            .exports
            .iter()
            .map(|(k, v)| Item {
                name: resolve.name_world_key(k),
                kind: kind(v).into(),
                asynchronous: matches!(v,wit_parser::WorldItem::Function(f) if f.kind.is_async()),
            })
            .collect();
        let wit = super::wit_print::print(&resolve, world)?;
        Ok(Self {
            imports,
            exports,
            wit,
            unsupported: None,
            resolve: Some(resolve),
            world: Some(world),
        })
    }
}

fn kind(v: &wit_parser::WorldItem) -> &'static str {
    match v {
        wit_parser::WorldItem::Function(_) => "func",
        wit_parser::WorldItem::Interface { .. } => "instance",
        wit_parser::WorldItem::Type { .. } => "type",
    }
}

pub(super) fn validate_metadata(bytes: &[u8]) -> Result<Option<(String, String)>, Error> {
    let mut recorded = None;
    let mut depth = 0;
    for p in wasmparser::Parser::new(0).parse_all(bytes) {
        match p.map_err(error)? {
            wasmparser::Payload::Version { .. } => depth += 1,
            wasmparser::Payload::End(_) => depth -= 1,
            wasmparser::Payload::CustomSection(s) if depth == 1 && s.name() == "walz:world-v1" => {
                #[derive(serde::Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Names {
                    package: String,
                    world: String,
                }
                if recorded.is_some() {
                    return Err(error("duplicate world metadata"));
                }
                let n: Names = serde_json::from_slice(s.data()).map_err(error)?;
                recorded = Some((n.package, n.world));
            }
            _ => {}
        }
    }
    if let Some((package, name)) = &recorded {
        let mut resolve = wit_parser::Resolve::default();
        let id = resolve
            .push_str(
                "world.wit",
                &format!("package {package}; world {name} {{}}"),
            )
            .map_err(error)?;
        if resolve.packages[id].name.to_string() != *package
            || resolve.packages[id].worlds.len() != 1
            || !resolve.packages[id].worlds.contains_key(name)
            || !resolve.packages[id].interfaces.is_empty()
        {
            return Err(error("invalid package or world name"));
        }
    }
    Ok(recorded)
}
