//! Checked mapping from flattened implementation exports to an original WIT world.
use super::abi::component_type;
use crate::{Error, ir::*};
use std::{collections::BTreeMap, path::Path};
use wit_parser::{Resolve, Type as WType, TypeDefKind as WKind, WorldItem};

mod resources;

fn fail(message: impl Into<String>) -> Error {
    Error::message(message.into()).with_code("E_WIT_WORLD")
}
pub(super) fn selected_exports(
    path: &Path,
    name: Option<&str>,
) -> Result<std::collections::BTreeSet<String>, Error> {
    let mut resolve = Resolve::default();
    let (package, _) = resolve.push_path(path).map_err(|e| fail(e.to_string()))?;
    let id = resolve
        .select_world(&[package], name)
        .map_err(|e| fail(e.to_string()))?;
    let mut names = std::collections::BTreeSet::new();
    for item in resolve.worlds[id].exports.values() {
        match item {
            WorldItem::Interface { id, .. } => {
                let interface = &resolve.interfaces[*id];
                for f in interface.functions.values() {
                    names.insert(format!(
                        "{}-{}",
                        interface.name.as_deref().unwrap_or("api"),
                        f.name
                    ));
                }
            }
            WorldItem::Function(f) => {
                names.insert(f.name.clone());
            }
            WorldItem::Type { .. } => return Err(fail("world-level type exports are unsupported")),
        }
    }
    Ok(names)
}
enum Export {
    Function {
        external: String,
        implementation: String,
    },
    Interface {
        external: String,
        types: Vec<(String, wit_parser::TypeId)>,
        functions: Vec<(String, String)>,
    },
}
pub(super) struct World {
    resolve: Resolve,
    exports: Vec<Export>,
    types: BTreeMap<wit_parser::TypeId, Type>,
    names: BTreeMap<String, Vec<String>>,
    asyncs: std::collections::BTreeSet<String>,
    resources: BTreeMap<TypeId, resources::Resource>,
}
impl World {
    pub(super) fn load(path: &Path, name: Option<&str>, module: &Module) -> Result<Self, Error> {
        let mut resolve = Resolve::default();
        let (package, _) = resolve.push_path(path).map_err(|e| fail(e.to_string()))?;
        let id = resolve
            .select_world(&[package], name)
            .map_err(|e| fail(e.to_string()))?;
        let mut this = Self {
            resolve,
            exports: vec![],
            types: BTreeMap::new(),
            names: BTreeMap::new(),
            asyncs: Default::default(),
            resources: Default::default(),
        };
        let world = this.resolve.worlds[id].clone();
        this.discover_resources(module, &world)?;
        for (key, item) in world.exports {
            let external = this.resolve.name_world_key(&key);
            match item {
                WorldItem::Interface { id, .. } => {
                    let interface = this.resolve.interfaces[id].clone();
                    let prefix = interface.name.as_deref().unwrap_or("api");
                    let mut functions = vec![];
                    for f in interface.functions.values() {
                        let implementation = format!("{prefix}-{}", f.name);
                        this.function(module, &implementation, f)?;
                        functions.push((f.name.clone(), implementation));
                    }
                    this.exports.push(Export::Interface {
                        external,
                        types: interface.types.into_iter().collect(),
                        functions,
                    });
                }
                WorldItem::Function(f) => {
                    this.function(module, &f.name, &f)?;
                    this.exports.push(Export::Function {
                        external,
                        implementation: f.name,
                    });
                }
                WorldItem::Type { .. } => {
                    return Err(fail("world-level type exports are unsupported"));
                }
            }
        }
        // Dead imports are already projected out. Every remaining host call
        // must be part of the selected world, with its separately checked policy.
        for f in &module.functions {
            let FunctionKind::Import {
                module: namespace,
                name,
            } = &f.kind
            else {
                continue;
            };
            if this.check_close(module, f)? {
                continue;
            }
            if f.import_contract
                .as_ref()
                .is_some_and(|b| b.resource.is_some())
            {
                return Err(fail(
                    "native WIT worlds require typed own/borrow imports; regenerate numeric resource bindings with walz bind",
                ));
            }
            let expected = world
                .imports
                .iter()
                .find_map(|(key, item)| match item {
                    WorldItem::Interface { id, .. }
                        if this.resolve.name_world_key(key) == *namespace =>
                    {
                        this.resolve.interfaces[*id].functions.get(name).cloned()
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    fail(format!(
                        "import {namespace}#{name} is outside the WIT world"
                    ))
                })?;
            this.signature(module, f, &expected)?;
            this.names.insert(
                format!("{namespace}#{name}"),
                expected.params.iter().map(|p| p.name.clone()).collect(),
            );
        }
        Ok(this)
    }
    fn function(
        &mut self,
        module: &Module,
        name: &str,
        expected: &wit_parser::Function,
    ) -> Result<(), Error> {
        let f = module
            .functions
            .iter()
            .find(|f| f.exports.iter().any(|n| n == name))
            .ok_or_else(|| fail(format!("missing WIT implementation export {name}")))?;
        self.signature(module, f, expected)?;
        if expected.kind.is_async() {
            self.asyncs.insert(name.into());
        }
        self.names.insert(
            name.into(),
            expected.params.iter().map(|p| p.name.clone()).collect(),
        );
        Ok(())
    }
    fn signature(
        &mut self,
        module: &Module,
        f: &Function,
        expected: &wit_parser::Function,
    ) -> Result<(), Error> {
        let async_ = expected.kind.is_async();
        let suspends = module.abi_suspends.get(&f.name).copied().unwrap_or(false);
        if suspends && !async_ {
            return Err(fail(format!(
                "suspending implementation requires async WIT: {}",
                f.name
            )));
        }
        if f.params.len() != expected.params.len() {
            return Err(fail(format!("WIT parameter count differs for {}", f.name)));
        }
        for (local, param) in f.params.iter().zip(&expected.params) {
            self.bind(module, param.ty, &f.locals[local.0].ty)?;
        }
        match expected.result {
            Some(t) => self.bind(module, t, &f.result)?,
            None if f.result == Type::Unit => {}
            _ => return Err(fail(format!("WIT result differs for {}", f.name))),
        }
        Ok(())
    }
    fn bind(&mut self, module: &Module, expected: WType, actual: &Type) -> Result<(), Error> {
        if self.bind_resource(expected, actual)? {
            return Ok(());
        }
        let expected_shape = crate::wit_bindgen::contract_shape(&self.resolve, expected)?;
        if expected_shape != module.shape_name(actual) {
            return Err(fail(format!(
                "WIT type mismatch: expected {expected_shape}, got {}",
                module.shape_name(actual)
            )));
        }
        let WType::Id(id) = expected else {
            return Ok(());
        };
        if let Some(old) = self.types.insert(id, actual.clone())
            && old != *actual
        {
            return Err(fail("one WIT type maps to different nominal source types"));
        }
        let value = match actual {
            Type::Value(id) => Some(&module.types[id.0].kind),
            _ => None,
        };
        match self.resolve.types[id].kind.clone() {
            WKind::Type(t) => self.bind(module, t, actual)?,
            WKind::Record(r) => {
                let Type::Ref { ty, .. } = actual else {
                    return Err(fail("expected record storage"));
                };
                let TypeKind::Struct(fields) = &module.types[ty.0].kind else {
                    unreachable!()
                };
                for (f, a) in r.fields.iter().zip(fields) {
                    self.bind(module, f.ty, &a.ty)?;
                }
            }
            WKind::Variant(v) => {
                let Type::Ref { ty, .. } = actual else {
                    return Err(fail("expected variant storage"));
                };
                let TypeKind::Variant(cases) = &module.types[ty.0].kind else {
                    unreachable!()
                };
                for (c, a) in v.cases.iter().zip(cases) {
                    if let Some(t) = c.ty {
                        self.bind(module, t, &a.payloads[0])?;
                    }
                }
            }
            WKind::Tuple(t) => {
                let fields = match value {
                    Some(TypeKind::Value(ValueType::Pair { first, second })) => {
                        vec![first.clone(), second.clone()]
                    }
                    _ => {
                        let Type::Ref { ty, .. } = actual else {
                            return Err(fail("expected tuple storage"));
                        };
                        let TypeKind::Struct(f) = &module.types[ty.0].kind else {
                            unreachable!()
                        };
                        f.iter().map(|f| f.ty.clone()).collect()
                    }
                };
                for (t, a) in t.types.iter().zip(fields) {
                    self.bind(module, *t, &a)?;
                }
            }
            WKind::List(_) if matches!(value, Some(TypeKind::Value(ValueType::Bytes))) => {}
            WKind::List(t) | WKind::Option(t) => {
                let Some(TypeKind::Value(ValueType::List(a) | ValueType::Option(a))) = value else {
                    unreachable!()
                };
                self.bind(module, t, a)?;
            }
            WKind::Result(r) => {
                let Some(TypeKind::Value(ValueType::Result { ok, err })) = value else {
                    unreachable!()
                };
                if let Some(t) = r.ok {
                    self.bind(module, t, ok)?;
                }
                if let Some(t) = r.err {
                    self.bind(module, t, err)?;
                }
            }
            WKind::Future(t) | WKind::Stream(t) => {
                let Some(TypeKind::Value(ValueType::Future(a) | ValueType::Stream(a))) = value
                else {
                    unreachable!()
                };
                if let Some(t) = t {
                    self.bind(module, t, a)?;
                }
            }
            WKind::Resource | WKind::Handle(_) => {
                return Err(fail(
                    "native world adapter does not yet support async endpoints or resource ABI",
                ));
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn imported_names(&self, namespace: &str, name: &str) -> Option<&[String]> {
        self.names
            .get(&format!("{namespace}#{name}"))
            .map(Vec::as_slice)
    }
    pub(super) fn is_async(&self, name: &str) -> bool {
        self.asyncs.contains(name)
    }
    pub(super) fn names(&self, export: &str) -> Option<&[String]> {
        self.names.get(export).map(Vec::as_slice)
    }
    pub(super) fn render(
        &self,
        functions: &BTreeMap<String, String>,
    ) -> Result<(String, String), Error> {
        let mut types = TypeWriter {
            world: self,
            defined: BTreeMap::new(),
            declarations: String::new(),
            import_namespace: None,
            prefix: "world_".into(),
        };
        let mut body = String::new();
        let mut outer = String::new();
        for (i, export) in self.exports.iter().enumerate() {
            match export {
                Export::Function {
                    external,
                    implementation,
                } => {
                    body.push_str(&format!(
                        "(export {external:?} (func {}))\n",
                        functions[implementation]
                    ));
                    outer.push_str(&format!("(alias export $api {external:?} (func $world_f{i}))\n(export {external:?} (func $world_f{i}))\n"));
                }
                Export::Interface {
                    external,
                    types: names,
                    functions: items,
                } => {
                    let mut exports = String::new();
                    for (name, id) in names {
                        exports.push_str(&format!(
                            "(export {name:?} (type {}))\n",
                            types.named(*id, 0)?
                        ));
                    }
                    for (name, implementation) in items {
                        exports.push_str(&format!(
                            "(export {name:?} (func {}))\n",
                            functions[implementation]
                        ));
                    }
                    body.push_str(&format!("(instance $world_i{i} {exports})\n(export {external:?} (instance $world_i{i}))\n"));
                    outer.push_str(&format!("(alias export $api {external:?} (instance $world_i{i}))\n(export {external:?} (instance $world_i{i}))\n"));
                }
            }
        }
        Ok((format!("{}{body}", types.declarations), outer))
    }
}
struct TypeWriter<'a> {
    world: &'a World,
    defined: BTreeMap<wit_parser::TypeId, String>,
    declarations: String,
    import_namespace: Option<String>,
    prefix: String,
}
impl TypeWriter<'_> {
    fn ty(&mut self, t: WType, depth: usize) -> Result<String, Error> {
        Ok(match t {
            WType::Id(id) => return self.named(id, depth + 1),
            WType::Bool => "bool",
            WType::Char => "char",
            WType::S8 => "s8",
            WType::S16 => "s16",
            WType::S32 => "s32",
            WType::S64 => "s64",
            WType::U8 => "u8",
            WType::U16 => "u16",
            WType::U32 => "u32",
            WType::U64 => "u64",
            WType::F32 => "f32",
            WType::F64 => "f64",
            WType::String => "string",
            _ => return Err(fail("unsupported WIT value")),
        }
        .into())
    }
    fn named(&mut self, id: wit_parser::TypeId, depth: usize) -> Result<String, Error> {
        if depth > 64 {
            return Err(fail("recursive or excessively nested WIT type"));
        }
        if self.import_namespace.is_none()
            && let Some(Type::Resource(r)) = self.world.types.get(&id)
            && matches!(self.world.resolve.types[id].kind, WKind::Resource)
        {
            return Ok(format!("$t{}", r.0));
        }
        if self.import_namespace.is_none()
            && let Some(t) = self.world.types.get(&id)
        {
            let t = component_type(t);
            if t.starts_with('$') {
                return Ok(t);
            }
        }
        if let Some(name) = self.defined.get(&id) {
            return Ok(name.clone());
        }
        let name = format!("${}t{}", self.prefix, id.index());
        if matches!(self.world.resolve.types[id].kind, WKind::Resource) {
            if let Some(ns) = &self.import_namespace {
                if let Some((source, r)) = self.world.resources.iter().find(|(_, r)| r.wit == id)
                    && r.namespace != *ns
                {
                    self.declarations += &format!(
                        "(alias outer 1 $native_resource{} (type {name}))\n",
                        source.0
                    );
                } else {
                    let resource = self
                        .world
                        .resources
                        .values()
                        .find(|r| r.wit == id)
                        .map(|r| r.name.as_str())
                        .or(self.world.resolve.types[id].name.as_deref())
                        .ok_or_else(|| fail("unnamed WIT resource"))?;
                    self.declarations +=
                        &format!("(export {resource:?} (type {name} (sub resource)))\n");
                }
                self.defined.insert(id, name.clone());
                return Ok(name);
            }
            return Err(fail(
                "native worlds currently require imported WIT resources",
            ));
        }
        let def = match self.world.resolve.types[id].kind.clone() {
            WKind::Type(WType::Id(other)) => return self.named(other, depth + 1),
            WKind::Type(t) => self.ty(t, depth + 1)?,
            WKind::Record(r) => format!(
                "(record {})",
                r.fields
                    .iter()
                    .map(|f| Ok(format!(
                        "(field {:?} {})",
                        f.name,
                        self.ty(f.ty, depth + 1)?
                    )))
                    .collect::<Result<Vec<_>, Error>>()?
                    .join(" ")
            ),
            WKind::Tuple(t) => format!(
                "(tuple {})",
                t.types
                    .iter()
                    .map(|t| self.ty(*t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" ")
            ),
            WKind::Variant(v) => format!(
                "(variant {})",
                v.cases
                    .iter()
                    .map(|c| Ok(format!(
                        "(case {:?}{})",
                        c.name,
                        c.ty.map(|t| self.ty(t, depth + 1).map(|t| format!(" {t}")))
                            .transpose()?
                            .unwrap_or_default()
                    )))
                    .collect::<Result<Vec<_>, Error>>()?
                    .join(" ")
            ),
            WKind::Enum(e) => format!(
                "(enum {})",
                e.cases
                    .iter()
                    .map(|c| format!("{:?}", c.name))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            WKind::Flags(f) => format!(
                "(flags {})",
                f.flags
                    .iter()
                    .map(|f| format!("{:?}", f.name))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            WKind::List(t) => format!("(list {})", self.ty(t, depth + 1)?),
            WKind::Option(t) => format!("(option {})", self.ty(t, depth + 1)?),
            WKind::Result(r) => format!(
                "(result {}{})",
                r.ok.map(|t| self.ty(t, depth + 1))
                    .transpose()?
                    .unwrap_or_default(),
                r.err
                    .map(|t| self.ty(t, depth + 1).map(|t| format!(" (error {t})")))
                    .transpose()?
                    .unwrap_or_default()
            ),
            WKind::Handle(wit_parser::Handle::Own(r)) => {
                format!("(own {})", self.named(r, depth + 1)?)
            }
            WKind::Handle(wit_parser::Handle::Borrow(r)) => {
                format!("(borrow {})", self.named(r, depth + 1)?)
            }
            WKind::Future(t) | WKind::Stream(t) => format!(
                "({}{})",
                if matches!(self.world.resolve.types[id].kind, WKind::Future(_)) {
                    "future"
                } else {
                    "stream"
                },
                t.map(|t| self.ty(t, depth + 1).map(|t| format!(" {t}")))
                    .transpose()?
                    .unwrap_or_default()
            ),
            _ => {
                return Err(fail(
                    "native world adapter does not yet support resource ABI",
                ));
            }
        };
        let name = format!("${}t{}", self.prefix, id.index());
        self.declarations
            .push_str(&format!("(type {name} {def})\n"));
        self.defined.insert(id, name.clone());
        Ok(name)
    }
}
