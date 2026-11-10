//! WIT supplies shapes. A separate, host-owned policy supplies authority.
use crate::{
    Error,
    bindings::{Binding, BindingPolicy, PolicyImport},
};
use std::path::Path;
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem};

#[derive(Debug)]
pub struct Generated {
    pub source: String,
    pub policy: String,
    pub linked_source: String,
    pub types_source: String,
    pub export_source: String,
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
    let mut schema = Schema::new(&resolve);
    for item in resolve.worlds[world].imports.values() {
        let WorldItem::Interface { id, .. } = item else {
            continue;
        };
        let Some(ns) = resolve.id_of(*id) else {
            continue;
        };
        for (name, t) in &resolve.interfaces[*id].types {
            if let Ok(resource) = canonical_resource(&resolve, *t) {
                let direct = matches!(resolve.types[resource].owner,wit_parser::TypeOwner::Interface(owner) if owner==*id);
                if direct || !schema.resource_anchors.contains_key(&resource) {
                    schema
                        .resource_anchors
                        .insert(resource, (ns.clone(), name.clone()));
                }
            }
        }
    }
    let mut export_functions = String::new();
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
            let async_ = function.kind.is_async();
            if async_ != grant.suspends {
                return Err(fail(format!(
                    "policy suspension disagrees with WIT for {}",
                    function.name
                )));
            }
            let params = function
                .params
                .iter()
                .map(|p| schema.shape(p.ty, 0, ""))
                .collect::<Result<Vec<_>, _>>()?;
            let result = function
                .result
                .map(|t| schema.shape(t, 0, ""))
                .transpose()?
                .unwrap_or("unit".into());
            let item = function
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                        c
                    } else {
                        '-'
                    }
                })
                .collect::<String>();
            let local = format!("${}.{}", interface.name.as_deref().unwrap(), item);
            if !names.insert(local.clone()) {
                return Err(fail(format!("duplicate local name {local}")));
            }
            source.push_str(&format!(
                "  (import {namespace:?} {:?} (func {local}",
                function.name
            ));
            for param in &function.params {
                let ty = schema.shape(param.ty, 0, "$")?;
                standard_types.extend(
                    ty.split(['(', ')', ' '])
                        .filter(|t| {
                            matches!(
                                *t,
                                "$string"
                                    | "$bytes"
                                    | "$list"
                                    | "$tuple"
                                    | "$option"
                                    | "$result"
                                    | "$future"
                                    | "$stream"
                                    | "$unit"
                            )
                        })
                        .map(str::to_owned),
                );
                source.push_str(&format!(" (param ${} {ty})", param.name));
            }
            if let Some(result) = function.result {
                let ty = schema.shape(result, 0, "$")?;
                standard_types.extend(
                    ty.split(['(', ')', ' '])
                        .filter(|t| {
                            matches!(
                                *t,
                                "$string"
                                    | "$bytes"
                                    | "$list"
                                    | "$tuple"
                                    | "$option"
                                    | "$result"
                                    | "$future"
                                    | "$stream"
                                    | "$unit"
                            )
                        })
                        .map(str::to_owned),
                );
                source.push_str(&format!(" (result {ty})"));
            }
            source.push_str(&format!("))\n  (@export (func {local}))\n"));
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
    for item in resolve.worlds[world].exports.values() {
        let (prefix, functions) = match item {
            WorldItem::Interface { id, .. } => (
                resolve.interfaces[*id]
                    .name
                    .as_deref()
                    .unwrap_or("api")
                    .to_owned(),
                resolve.interfaces[*id]
                    .functions
                    .values()
                    .collect::<Vec<_>>(),
            ),
            WorldItem::Function(f) => (String::new(), vec![f]),
            WorldItem::Type { .. } => continue,
        };
        for function in functions {
            let name = if prefix.is_empty() {
                function.name.clone()
            } else {
                format!("{prefix}-{}", function.name)
            };
            let params = function
                .params
                .iter()
                .map(|p| {
                    Ok(format!(
                        " (param ${} {})",
                        p.name,
                        schema.shape(p.ty, 0, "$")?
                    ))
                })
                .collect::<Result<String, Error>>()?;
            let result = function
                .result
                .map(|t| schema.shape(t, 0, "$").map(|t| format!(" (result {t})")))
                .transpose()?
                .unwrap_or_default();
            let local = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                        c
                    } else {
                        '-'
                    }
                })
                .collect::<String>();
            export_functions.push_str(&format!("  ;; Replace unreachable with the implementation.\n  (func ${local} (export {name:?}){params}{result} (@effect pure) (unreachable))\n"));
        }
    }
    for (module, name, kind) in &schema.drops {
        policy.imports.push(PolicyImport {
            module: module.clone(),
            name: name.clone(),
            binding: Binding {
                params: vec!["i32".into()],
                result: "unit".into(),
                suspends: false,
                capability: format!("resource-drop:{kind}"),
                resource: Some(crate::bindings::ResourceBinding {
                    kind: kind.clone(),
                    operation: crate::bindings::ResourceOperation::Close,
                }),
                param_names: vec!["handle".into()],
            },
        });
    }
    source.push_str(")\n");
    standard_types.extend(schema.standard.iter().cloned());
    let imports = standard_types
        .iter()
        .map(|name| format!(" (type {name})"))
        .collect::<String>();
    let core = if imports.is_empty() {
        String::new()
    } else {
        format!("  (@use \"walz:core\"{imports})\n")
    };
    let defs = schema.declarations.join("");
    let type_exports = schema
        .names
        .iter()
        .map(|n| format!(" (type {n})"))
        .collect::<String>();
    let public = if type_exports.is_empty() {
        String::new()
    } else {
        format!("  (@export{type_exports})\n")
    };
    let type_use = if type_exports.is_empty() {
        String::new()
    } else {
        format!("  (@use \"./types.walz\"{type_exports})\n")
    };
    let start = source.find("(module\n").unwrap() + "(module\n".len();
    let mut linked_source = source.clone();
    linked_source.insert_str(start, &format!("{core}{type_use}{public}"));
    source.insert_str(start, &format!("{core}{defs}{public}"));
    let types_source =
        format!(";; Generated nominal portable types.\n(module\n{core}{defs}{public})\n");
    let export_source = format!(
        ";; Generated export scaffold. Bodies must be implemented.\n(module\n{core}{type_use}{export_functions})\n"
    );
    let policy = serde_json::to_string_pretty(&policy).unwrap();
    BindingPolicy::parse(&policy)?;
    Ok(Generated {
        source,
        policy,
        linked_source,
        types_source,
        export_source,
    })
}

/// Decode exactly the same value shapes as generated host bindings. Foreign
/// component bodies remain opaque; WIT does not assert an effect contract.
pub(crate) fn component_exports(
    resolve: &Resolve,
    world: wit_parser::WorldId,
) -> Result<crate::binary::Interface, Error> {
    let mut schema = Schema::new(resolve);
    let mut exports = Vec::new();
    let mut unavailable = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for (key, item) in &resolve.worlds[world].exports {
        let outer = resolve.name_world_key(key);
        let functions = match item {
            WorldItem::Function(f) => vec![(format!("${}", f.name), vec![outer], f)],
            WorldItem::Interface { id, .. } => resolve.interfaces[*id]
                .functions
                .values()
                .map(|f| {
                    (
                        format!(
                            "${}.{}",
                            resolve.interfaces[*id].name.as_deref().unwrap_or(&outer),
                            f.name
                        ),
                        vec![outer.clone(), f.name.clone()],
                        f,
                    )
                })
                .collect(),
            WorldItem::Type { .. } => continue,
        };
        for (selector, path, function) in functions {
            if !names.insert(selector.clone()) {
                return Err(Error::message(format!(
                    "ambiguous component export {selector}"
                )));
            }
            if !component_value_limits(resolve, function) {
                unavailable.push(crate::binary::Unavailable {
                    selector,
                    reason: "WIT value graph exceeds expanded shape/depth limits".into(),
                });
                continue;
            }
            let owned = function
                .params
                .iter()
                .any(|p| contains_owned(resolve, p.ty, 0))
                || function
                    .result
                    .is_some_and(|t| contains_owned(resolve, t, 0));
            if function.kind.is_async() || owned {
                unavailable.push(crate::binary::Unavailable { selector, reason:
                    "Component @use currently accepts synchronous shared-value exports; async functions, future, stream and resource need a cross-component cancellation/lifetime contract".into() });
                continue;
            }
            let params = function
                .params
                .iter()
                .map(|p| schema.shape(p.ty, 0, ""))
                .collect::<Result<Vec<_>, _>>()?;
            let result = function
                .result
                .map(|t| schema.shape(t, 0, ""))
                .transpose()?
                .unwrap_or("unit".into());
            let mut declaration = String::new();
            for (i, p) in function.params.iter().enumerate() {
                declaration.push_str(&format!(" (param $p{i} {})", schema.shape(p.ty, 0, "$")?));
            }
            if let Some(result) = function.result {
                declaration.push_str(&format!(" (result {})", schema.shape(result, 0, "$")?));
            }
            exports.push(crate::binary::Export {
                selector,
                path,
                declaration,
                binding: Binding {
                    params,
                    result,
                    suspends: function.kind.is_async(),
                    capability: "component".into(),
                    param_names: function.params.iter().map(|p| p.name.clone()).collect(),
                    resource: None,
                },
            });
        }
    }
    if !schema.drops.is_empty() {
        return Err(Error::message(
            "component @use does not yet transfer resource handles".into(),
        ));
    }
    let core = schema
        .standard
        .iter()
        .map(|n| format!(" (type {n})"))
        .collect::<String>();
    let types = schema
        .names
        .iter()
        .map(|n| format!(" (type {n})"))
        .collect::<String>();
    Ok(crate::binary::Interface {
        source: format!(
            "{}{}{}",
            if core.is_empty() {
                String::new()
            } else {
                format!("(@use \"walz:core\"{core})\n")
            },
            schema.declarations.join(""),
            if types.is_empty() {
                String::new()
            } else {
                format!("(@export{types})\n")
            },
        ),
        exports,
        unavailable,
    })
}

// Count expanded paths, not distinct type IDs: a compact DAG can have an
// exponentially large shape. Stop before contains_owned or shape expands it.
fn component_value_limits(resolve: &Resolve, function: &wit_parser::Function) -> bool {
    fn charge(budget: &mut usize, amount: usize) -> bool {
        if let Some(left) = budget.checked_sub(amount) {
            *budget = left;
            true
        } else {
            false
        }
    }
    fn visit(r: &Resolve, ty: Type, depth: usize, budget: &mut usize) -> bool {
        if depth > 64 || !charge(budget, 32) {
            return false;
        }
        let Type::Id(id) = ty else {
            return true;
        };
        match &r.types[id].kind {
            TypeDefKind::Type(t) | TypeDefKind::List(t) | TypeDefKind::Option(t) => {
                visit(r, *t, depth + 1, budget)
            }
            TypeDefKind::Tuple(t) => t.types.iter().all(|t| visit(r, *t, depth + 1, budget)),
            TypeDefKind::Record(t) => t
                .fields
                .iter()
                .all(|f| charge(budget, f.name.len()) && visit(r, f.ty, depth + 1, budget)),
            TypeDefKind::Variant(t) => t.cases.iter().all(|c| {
                charge(budget, c.name.len()) && c.ty.is_none_or(|t| visit(r, t, depth + 1, budget))
            }),
            TypeDefKind::Result(t) => {
                t.ok.into_iter()
                    .chain(t.err)
                    .all(|t| visit(r, t, depth + 1, budget))
            }
            TypeDefKind::Enum(t) => t.cases.iter().all(|c| charge(budget, c.name.len())),
            TypeDefKind::Flags(t) => t.flags.iter().all(|f| charge(budget, f.name.len())),
            _ => true,
        }
    }
    let mut budget = 64 * 1024;
    function
        .params
        .iter()
        .map(|p| p.ty)
        .chain(function.result)
        .all(|t| visit(resolve, t, 0, &mut budget))
}

struct Schema<'a> {
    r: &'a Resolve,
    declarations: Vec<String>,
    names: std::collections::BTreeSet<String>,
    standard: std::collections::BTreeSet<String>,
    drops: Vec<(String, String, String)>,
    resource_anchors: std::collections::BTreeMap<wit_parser::TypeId, (String, String)>,
}
impl<'a> Schema<'a> {
    fn new(r: &'a Resolve) -> Self {
        Self {
            r,
            declarations: vec![],
            names: Default::default(),
            standard: Default::default(),
            drops: vec![],
            resource_anchors: Default::default(),
        }
    }
    fn ensure_resource(&mut self, mut id: wit_parser::TypeId) -> Result<String, Error> {
        for _ in 0..64 {
            if let TypeDefKind::Type(Type::Id(other)) = self.r.types[id].kind {
                id = other;
            } else {
                break;
            }
        }
        let name = self.type_name(id);
        if self.names.insert(name.clone()) {
            let kind = resource_kind(self.r, id)?;
            let wit_parser::TypeOwner::Interface(owner) = self.r.types[id].owner else {
                return Err(Error::message(
                    "WIT resources require an interface owner".into(),
                ));
            };
            let namespace = self
                .r
                .id_of(owner)
                .ok_or_else(|| Error::message("anonymous resource interface".into()))?;
            let (namespace, resource_name) = self
                .resource_anchors
                .get(&id)
                .cloned()
                .unwrap_or_else(|| (namespace, self.r.types[id].name.clone().unwrap()));
            let drop = format!("[resource-drop]{resource_name}");
            let local = format!("$wit-drop-resource{}", id.index());
            self.declarations.push(format!("  (import {namespace:?} {drop:?} (func {local} (param $handle i32)))\n  (type {name} (@resource {kind:?} {local}))\n"));
            self.drops.push((namespace, drop, kind));
        }
        Ok(name)
    }
    fn unsigned(&mut self, bits: u32) -> String {
        let name = format!("$wit-u{bits}");
        if self.names.insert(name.clone()) {
            self.declarations.push(format!(
                "  (type {name} (@newtype i{}))\n  (@abi.type {name} (u{bits}))\n",
                bits.max(32)
            ));
        }
        name
    }
    fn primitive(&mut self, name: &str) -> String {
        let local = format!("$wit-{name}");
        if self.names.insert(local.clone()) {
            self.declarations.push(format!(
                "  (type {local} (@newtype i32))\n  (@abi.type {local} ({name}))\n"
            ));
        }
        local
    }
    fn type_name(&self, id: wit_parser::TypeId) -> String {
        let ty = &self.r.types[id];
        let interface = match ty.owner {
            wit_parser::TypeOwner::Interface(i) => {
                self.r.interfaces[i].name.as_deref().unwrap_or("wit")
            }
            _ => "wit",
        };
        format!(
            "${interface}.{}",
            ty.name
                .clone()
                .unwrap_or_else(|| format!("t{}", id.index()))
        )
    }
    fn ensure(&mut self, id: wit_parser::TypeId) -> Result<String, Error> {
        let name = self.type_name(id);
        if !self.names.insert(name.clone()) {
            return Ok(name);
        }
        let owned = match &self.r.types[id].kind {
            TypeDefKind::Record(r) => r.fields.iter().any(|f| contains_owned(self.r, f.ty, 0)),
            TypeDefKind::Variant(v) => v
                .cases
                .iter()
                .filter_map(|c| c.ty)
                .any(|t| contains_owned(self.r, t, 0)),
            TypeDefKind::Tuple(t) => t.types.iter().any(|t| contains_owned(self.r, *t, 0)),
            _ => false,
        };
        if owned {
            return Err(Error::message(
                "named ABI records, variants and tuples require shared payloads".into(),
            ));
        }
        let (definition, annotation) = match self.r.types[id].kind.clone() {
            TypeDefKind::Tuple(tuple) => {
                if tuple.types.is_empty() {
                    return Err(Error::message(
                        "WIT tuples require at least one element".into(),
                    ));
                }
                let fields = tuple
                    .types
                    .iter()
                    .enumerate()
                    .map(|(i, t)| Ok(format!(" (field $item{i} {})", self.shape(*t, 0, "$")?)))
                    .collect::<Result<String, Error>>()?;
                (format!("(struct{fields})"), "(tuple)".into())
            }
            TypeDefKind::Variant(variant) => {
                let cases = variant
                    .cases
                    .iter()
                    .map(|c| {
                        Ok(format!(
                            " ({}{})",
                            c.name,
                            c.ty.map(|t| self.shape(t, 0, "$").map(|t| format!(" {t}")))
                                .transpose()?
                                .unwrap_or_default()
                        ))
                    })
                    .collect::<Result<String, Error>>()?;
                let names = variant
                    .cases
                    .iter()
                    .map(|c| format!(" {:?}", c.name))
                    .collect::<String>();
                (format!("(@variant{cases})"), format!("(variant{names})"))
            }
            TypeDefKind::Record(record) => {
                if record.fields.is_empty() {
                    return Err(Error::message(
                        "WIT records require at least one field".into(),
                    ));
                }
                let fields = record
                    .fields
                    .iter()
                    .map(|f| {
                        Ok(format!(
                            " (field ${} {})",
                            f.name,
                            self.shape(f.ty, 0, "$")?
                        ))
                    })
                    .collect::<Result<String, Error>>()?;
                let names = record
                    .fields
                    .iter()
                    .map(|f| format!(" {:?}", f.name))
                    .collect::<String>();
                (format!("(struct{fields})"), format!("(record{names})"))
            }
            TypeDefKind::Enum(e) => {
                let cases = e
                    .cases
                    .iter()
                    .map(|c| format!(" ({})", c.name))
                    .collect::<String>();
                let names = e
                    .cases
                    .iter()
                    .map(|c| format!(" {:?}", c.name))
                    .collect::<String>();
                (format!("(@variant{cases})"), format!("(enum{names})"))
            }
            TypeDefKind::Flags(f) => {
                if f.flags.is_empty() || f.flags.len() > crate::interface::MAX_FLAGS {
                    return Err(Error::message(
                        "WIT flags require 1 to 32 flags for the pinned Component ABI".into(),
                    ));
                }
                let names = f
                    .flags
                    .iter()
                    .map(|f| format!(" {:?}", f.name))
                    .collect::<String>();
                ("(@newtype i32)".into(), format!("(flags{names})"))
            }
            _ => unreachable!(),
        };
        self.declarations.push(format!(
            "  (type {name} {definition})\n  (@abi.type {name} {annotation})\n"
        ));
        Ok(name)
    }
    fn optional(&mut self, t: Option<Type>, depth: usize, prefix: &str) -> Result<String, Error> {
        if let Some(t) = t {
            self.shape(t, depth + 1, prefix)
        } else {
            if prefix == "$" {
                self.standard.insert("$unit".into());
            }
            Ok(format!("{prefix}unit"))
        }
    }
    fn shape(&mut self, ty: Type, depth: usize, prefix: &str) -> Result<String, Error> {
        if depth > 64 {
            return Err(Error::message("WIT type nesting exceeds 64".into()));
        }
        let r = self.r;
        let constructor = |name: &str| {
            if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("@inst ${name}")
            }
        };
        Ok(match ty {
            Type::Bool | Type::Char | Type::S8 | Type::S16 => {
                let name = match ty {
                    Type::Bool => "bool",
                    Type::Char => "char",
                    Type::S8 => "s8",
                    _ => "s16",
                };
                if prefix == "$" {
                    self.primitive(name)
                } else {
                    name.into()
                }
            }
            Type::S32 => "i32".into(),
            Type::S64 => "i64".into(),
            Type::F32 => "f32".into(),
            Type::F64 => "f64".into(),
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                let bits = match ty {
                    Type::U8 => 8,
                    Type::U16 => 16,
                    Type::U32 => 32,
                    _ => 64,
                };
                if prefix == "$" {
                    self.unsigned(bits)
                } else {
                    format!("u{bits}")
                }
            }
            Type::String => {
                if prefix == "$" {
                    self.standard.insert("$string".into());
                    "$string".into()
                } else {
                    "text".into()
                }
            }
            Type::Id(id) => match r.types[id].kind.clone() {
                TypeDefKind::Type(t) => self.shape(t, depth + 1, prefix)?,
                TypeDefKind::Record(record) => {
                    if prefix == "$" {
                        format!("(ref {})", self.ensure(id)?)
                    } else {
                        let fields = record
                            .fields
                            .iter()
                            .map(|f| {
                                Ok(format!(
                                    "({} {})",
                                    f.name,
                                    self.shape(f.ty, depth + 1, prefix)?
                                ))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;
                        format!("(record {})", fields.join(" "))
                    }
                }
                TypeDefKind::Enum(e) => {
                    if prefix == "$" {
                        format!("(ref {})", self.ensure(id)?)
                    } else {
                        format!(
                            "(enum {})",
                            e.cases
                                .iter()
                                .map(|c| c.name.clone())
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    }
                }
                TypeDefKind::Flags(f) => {
                    if prefix == "$" {
                        self.ensure(id)?
                    } else {
                        format!(
                            "(flags {})",
                            f.flags
                                .iter()
                                .map(|f| f.name.clone())
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    }
                }
                TypeDefKind::List(Type::U8) => {
                    if prefix == "$" {
                        self.standard.insert("$bytes".into());
                    }
                    format!("{prefix}bytes")
                }
                TypeDefKind::List(t) => {
                    if contains_owned(r, t, 0) {
                        return Err(Error::message("unsupported WIT list: elements must be shared data; owned resources and endpoints are unsupported".into()));
                    }
                    if prefix == "$" {
                        self.standard.insert("$list".into());
                    }
                    format!(
                        "({} {})",
                        constructor("list"),
                        self.shape(t, depth + 1, prefix)?
                    )
                }
                TypeDefKind::Option(t) => {
                    if prefix == "$" {
                        self.standard.insert("$option".into());
                    }
                    format!(
                        "({} {})",
                        constructor("option"),
                        self.shape(t, depth + 1, prefix)?
                    )
                }
                TypeDefKind::Result(t) => {
                    if prefix == "$" {
                        self.standard.insert("$result".into());
                    }
                    format!(
                        "({} {} {})",
                        constructor("result"),
                        self.optional(t.ok, depth, prefix)?,
                        self.optional(t.err, depth, prefix)?
                    )
                }
                TypeDefKind::Variant(v) => {
                    if prefix == "$" {
                        format!("(ref {})", self.ensure(id)?)
                    } else {
                        format!(
                            "(variant {})",
                            v.cases
                                .iter()
                                .map(|c| Ok(format!(
                                    "({}{})",
                                    c.name,
                                    c.ty.map(|t| self
                                        .shape(t, depth + 1, prefix)
                                        .map(|t| format!(" {t}")))
                                        .transpose()?
                                        .unwrap_or_default()
                                )))
                                .collect::<Result<Vec<_>, Error>>()?
                                .join(" ")
                        )
                    }
                }
                TypeDefKind::Tuple(t) if t.types.len() != 2 => {
                    if prefix == "$" {
                        format!("(ref {})", self.ensure(id)?)
                    } else {
                        format!(
                            "(tuple{})",
                            t.types
                                .iter()
                                .map(|t| self.shape(*t, depth + 1, prefix).map(|t| format!(" {t}")))
                                .collect::<Result<String, Error>>()?
                        )
                    }
                }
                TypeDefKind::Tuple(t) if t.types.len() == 2 => {
                    if prefix == "$" {
                        self.standard.insert("$tuple".into());
                    }
                    format!(
                        "({} {} {})",
                        constructor(if prefix.is_empty() { "pair" } else { "tuple" }),
                        self.shape(t.types[0], depth + 1, prefix)?,
                        self.shape(t.types[1], depth + 1, prefix)?
                    )
                }
                TypeDefKind::Resource => {
                    let kind = resource_kind(r, id)?;
                    if prefix.is_empty() {
                        format!("(resource {kind})")
                    } else {
                        format!("(@resource {})", self.ensure_resource(id)?)
                    }
                }
                TypeDefKind::Handle(
                    wit_parser::Handle::Own(resource) | wit_parser::Handle::Borrow(resource),
                ) => {
                    let borrow = matches!(
                        r.types[id].kind,
                        TypeDefKind::Handle(wit_parser::Handle::Borrow(_))
                    );
                    let kind = resource_kind(r, resource)?;
                    if prefix.is_empty() {
                        format!("({} {kind})", if borrow { "borrow" } else { "resource" })
                    } else {
                        format!(
                            "(@{} {})",
                            if borrow { "borrow" } else { "resource" },
                            self.ensure_resource(resource)?
                        )
                    }
                }
                TypeDefKind::Future(t) => {
                    if t.is_some_and(|t| contains_owned(r, t, 0)) {
                        return Err(Error::message("future payload must be shared data".into()));
                    }

                    if prefix == "$" {
                        self.standard.insert("$future".into());
                    }
                    format!(
                        "({} {})",
                        constructor("future"),
                        self.optional(t, depth, prefix)?
                    )
                }
                TypeDefKind::Stream(t) => {
                    if t.is_some_and(|t| contains_owned(r, t, 0)) {
                        return Err(Error::message("stream payload must be shared data".into()));
                    }

                    if prefix == "$" {
                        self.standard.insert("$stream".into());
                    }
                    format!(
                        "({} {})",
                        constructor("stream"),
                        self.optional(t, depth, prefix)?
                    )
                }
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
}

pub(crate) fn canonical_resource(
    resolve: &Resolve,
    mut id: wit_parser::TypeId,
) -> Result<wit_parser::TypeId, Error> {
    for _ in 0..64 {
        match resolve.types[id].kind {
            TypeDefKind::Type(Type::Id(other)) => id = other,
            TypeDefKind::Resource => return Ok(id),
            _ => break,
        }
    }
    Err(Error::message("expected a WIT resource type".into()))
}

pub(crate) fn resource_kind(
    resolve: &Resolve,
    mut id: wit_parser::TypeId,
) -> Result<String, Error> {
    for _ in 0..64 {
        if let TypeDefKind::Type(Type::Id(other)) = resolve.types[id].kind {
            id = other;
            continue;
        }
        let wit_parser::TypeOwner::Interface(owner) = resolve.types[id].owner else {
            return Err(Error::message(
                "resource requires an interface owner".into(),
            ));
        };
        let ns = resolve
            .id_of(owner)
            .ok_or_else(|| Error::message("anonymous resource interface".into()))?;
        let name = resolve.types[id]
            .name
            .as_ref()
            .ok_or_else(|| Error::message("unnamed resource".into()))?;
        return Ok(format!("{ns}/{name}"));
    }
    Err(Error::message("cyclic resource alias".into()))
}

fn contains_owned(r: &Resolve, ty: Type, depth: usize) -> bool {
    if depth > 64 {
        return true;
    }
    let Type::Id(id) = ty else { return false };
    let sub = |t| contains_owned(r, t, depth + 1);
    match &r.types[id].kind {
        TypeDefKind::Future(_)
        | TypeDefKind::Stream(_)
        | TypeDefKind::Resource
        | TypeDefKind::Handle(_) => true,
        TypeDefKind::Type(t) | TypeDefKind::List(t) | TypeDefKind::Option(t) => sub(*t),
        TypeDefKind::Tuple(t) => t.types.iter().copied().any(sub),
        TypeDefKind::Result(t) => t.ok.into_iter().chain(t.err).any(sub),
        TypeDefKind::Record(r) => r.fields.iter().any(|f| sub(f.ty)),
        TypeDefKind::Variant(v) => v.cases.iter().filter_map(|c| c.ty).any(sub),
        _ => false,
    }
}

pub(crate) fn contract_shape(resolve: &Resolve, ty: Type) -> Result<String, Error> {
    Schema::new(resolve).shape(ty, 0, "")
}
