//! Private JSPI ABI. The guest shares the checked GC lowering and cancellation
//! protocol with Components; JS owns only boundary buffers and external ends.
use super::Adapter;
use super::abi::{align, type_key};
use crate::Error;
use crate::ir::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(serde::Serialize)]
pub struct Export {
    pub js_name: String,
    pub arity: usize,
}

mod values;
use values::{codec, read, write};

#[derive(serde::Serialize)]
pub struct CoreJs {
    #[serde(skip)]
    pub wat: String,
    pub wasm: Vec<u8>,
    pub memory: Vec<u8>,
    pub bindings: String,
    pub manifest: serde_json::Value,
    pub exports: BTreeMap<String, Export>,
}

pub fn compile(source: &str) -> Result<CoreJs, Error> {
    compile_with_policy(source, &crate::bindings::BindingPolicy::default())
}

/// Pure browser code generation: Rust type/effect/ownership and ABI checks,
/// with laws rejected because the external Lean kernel is unavailable.
pub fn compile_for_browser(source: &str, options: crate::CompileOptions) -> Result<CoreJs, Error> {
    compile_module(super::checked_browser_module(source, options.optimize)?)
}

pub fn compile_with_policy(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
) -> Result<CoreJs, Error> {
    compile_module(super::checked_module_with_policy(source, policy)?)
}

/// Host harness entry, selected only after checking the complete source module.
pub(crate) fn compile_case(
    source: &str,
    checked: &Module,
    function: FunctionId,
    export: &str,
) -> Result<CoreJs, Error> {
    let mut selected = checked.clone();
    selected.tests.clear();
    selected.benches.clear();
    for f in &mut selected.functions {
        f.exports.clear();
    }
    selected.functions[function.0].exports.push(export.into());
    // Harness declarations have inferred contracts. Publishing a private case
    // does not invent an effect promise: use the complete checked inference.
    if selected.functions[function.0].effect_contract.is_none() {
        selected.functions[function.0].effect_contract = Some(crate::effects::EffectContract {
            span: selected.functions[function.0]
                .body
                .as_ref()
                .map(|e| e.span)
                .unwrap_or_default(),
            allowed: crate::effects::infer(&selected)[function.0].clone(),
        });
    }
    compile_module(super::prepare_checked_module(
        source,
        selected,
        crate::verified::verify_component,
        true,
    )?)
}

fn validate_module(module: &Module) -> Result<(), Error> {
    crate::binary::core(module)?;
    if module
        .memories
        .iter()
        .any(|m| m.import.as_ref().is_some_and(|i| i.module == "__component"))
    {
        return Err(
            Error::message("__component is reserved for private ABI imports".into())
                .with_code("E_MEMORY_IMPORT"),
        );
    }
    Adapter::new(module).validate_boundary()
}

/// Check the same contracts and portable boundary as code generation, without
/// emitting Wasm or JavaScript. Used by read-only editor integrations.
pub(crate) fn metadata_with_policy(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
) -> Result<serde_json::Value, Error> {
    let module = super::checked_module_with_policy(source, policy)?;
    validate_module(&module)?;
    Ok(esm_metadata(&module))
}

fn esm_metadata(module: &Module) -> serde_json::Value {
    let boundary_effects = crate::effects::boundary(module);
    let mut exports = BTreeMap::new();
    for (i, function) in module.functions.iter().enumerate() {
        if module.tests.iter().any(|t| t.function.0 == i) {
            continue;
        }
        for name in &function.exports {
            exports.insert(name.clone(), serde_json::json!({
                "jsName": crate::bindings::js_name(name),
                "params": function.params.iter().map(|local| module.shape_name(&function.locals[local.0].ty)).collect::<Vec<_>>(),
                "result": module.shape_name(&function.result),
                "suspends": boundary_effects[i].suspends,
            }));
        }
    }
    serde_json::json!({"schema":"wlt-esm-v1", "exports":exports})
}

fn compile_module(module: Module) -> Result<CoreJs, Error> {
    validate_module(&module)?;
    let mut adapter = Adapter::new(&module);
    adapter.memory_exports = true;
    // Only boundary types and endpoint payloads need lifting/lowering helpers.
    let mut used = BTreeSet::new();
    for id in &adapter.endpoints {
        collect(&adapter, &Type::Value(*id), &mut used);
    }
    for (i, f) in module.functions.iter().enumerate() {
        if (!f.exports.is_empty() && !module.tests.iter().any(|t| t.function.0 == i))
            || matches!(f.kind, FunctionKind::Import { .. })
        {
            for id in &f.params {
                collect(&adapter, &f.locals[id.0].ty, &mut used);
            }
            collect(&adapter, &f.result, &mut used);
        }
    }
    adapter.types = used;
    let wat = crate::emit::emit_component(&module, &adapter);
    let wasm = parse(&wat)?;
    let memory = parse(&format!(
        "(module {})",
        include_str!("../component-memory.wat")
    ))?;
    let bindings = bindings(&adapter);
    let exports = module
        .functions
        .iter()
        .enumerate()
        .filter(|(i, _)| !module.tests.iter().any(|t| t.function.0 == *i))
        .flat_map(|(_, f)| {
            f.exports.iter().map(|name| {
                (
                    name.clone(),
                    Export {
                        js_name: crate::bindings::js_name(name),
                        arity: f.params.len(),
                    },
                )
            })
        })
        .collect();
    let mut manifest =
        serde_json::json!({"abi":"wlt-core-js-v2","imports":crate::bindings::manifest(&module)});
    manifest["esm"] = esm_metadata(&module);
    if module
        .functions
        .iter()
        .zip(crate::effects::boundary(&module))
        .any(|(f, e)| !f.exports.is_empty() && e.suspends)
    {
        manifest["asyncScopes"] = true.into();
    }
    if !adapter.endpoints.is_empty() {
        manifest["readables"] = true.into();
    }
    if crate::task_children::enabled(&module) {
        manifest["childTasks"] = true.into();
    }
    let memories = module.memories.iter().filter_map(|m| m.import.as_ref().map(|i| {
        serde_json::json!({"module":i.module,"name":i.name,"initial":m.initial,"maximum":m.maximum,"shared":m.shared})
    })).collect::<Vec<_>>();
    if !memories.is_empty() {
        manifest["memories"] = serde_json::json!(memories);
    }
    if let Some(certificate) = crate::race::certificate(&module)? {
        // Old runtimes must reject certified artifacts instead of ignoring the
        // host-contract handshake. The value codec is unchanged from v2.
        manifest["abi"] = serde_json::json!(if certificate.version == 2 {
            "wlt-core-js-v4"
        } else {
            "wlt-core-js-v3"
        });
        manifest["raceFree"] = serde_json::json!({
            "hostContract": if certificate.version == 2 { crate::readonly::HOST_CONTRACT } else { crate::race::HOST_CONTRACT },
            "hostRequired": module.memories.iter().any(|m| m.import.is_some()),
            "cells": certificate.cells,
            "kernel": "WltAtomicCells.race_free+uniform_cells",
        });
        if certificate.version == 2 {
            manifest["raceFree"]["readonlyRegions"] =
                serde_json::json!(certificate.readonly_regions);
            manifest["raceFree"]["kernel"] =
                serde_json::json!("WltAtomicCells.Readonly.race_free+readonly_in_bounds");
        }
    }
    Ok(CoreJs {
        wat,
        wasm,
        memory,
        bindings,
        manifest,
        exports,
    })
}

fn parse(wat: &str) -> Result<Vec<u8>, Error> {
    let wasm = wat::parse_str(wat).map_err(|e| Error::message(format!("Core JS WAT: {e}")))?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .map_err(|e| Error::message(format!("Core JS validation: {e}")))?;
    Ok(wasm)
}

fn collect(adapter: &Adapter<'_>, ty: &Type, used: &mut BTreeSet<Type>) {
    if !used.insert(ty.clone()) {
        return;
    }
    if let Type::Ref { ty: id, .. } = ty
        && let TypeKind::Struct(fields) = &adapter.module.types[id.0].kind
    {
        for field in fields {
            collect(adapter, &field.ty, used);
        }
    }
    if let Type::Ref { ty: id, .. } = ty
        && adapter.module.types[id.0].abi.is_some()
        && let TypeKind::Variant(cases) = &adapter.module.types[id.0].kind
    {
        for payload in cases.iter().flat_map(|c| &c.payloads) {
            collect(adapter, payload, used);
        }
    }
    if let Type::Value(id) = ty {
        match adapter.value(*id) {
            ValueType::List(t)
            | ValueType::Option(t)
            | ValueType::Future(t)
            | ValueType::Stream(t)
            | ValueType::Task(t) => collect(adapter, t, used),
            ValueType::Result { ok, err } => {
                collect(adapter, ok, used);
                collect(adapter, err, used);
            }
            ValueType::Pair { first, second } => {
                collect(adapter, first, used);
                collect(adapter, second, used);
            }
            _ => {}
        }
    }
}

fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

fn bindings(adapter: &Adapter<'_>) -> String {
    let effects = crate::effects::boundary(adapter.module);
    let mut out = String::from(
        "// Generated typed boundary functions; GC references stay inside Wasm.\nexport function bind(c) {\n",
    );
    out.push_str(values::prelude(adapter));
    for ty in &adapter.types {
        let (r, w) = codec(adapter, ty);
        out.push_str(&format!(
            "function r_{}(p){{{r}}}\nfunction w_{}(p,v){{{w}}}\n",
            type_key(ty),
            type_key(ty)
        ));
    }
    for id in &adapter.endpoints {
        let (kind, payload) = match adapter.value(*id) {
            ValueType::Future(t) => ("future", t),
            ValueType::Stream(t) => ("stream", t),
            _ => unreachable!(),
        };
        let r = if *payload == Type::Unit {
            "undefined".into()
        } else {
            read(payload, "p")
        };
        let w = if *payload == Type::Unit {
            "c.view().setUint8(p,0);".into()
        } else {
            write(payload, "p", "v")
        };
        out.push_str(&format!(
            "c.endpoint({},'{kind}',{},p=>{r},(p,v)=>{{{w}}});\n",
            id.0,
            adapter.layout(payload).size
        ));
    }
    out.push_str("const api=Object.create(null);\nconst asyncExports=[];\n");
    for (i, f) in adapter.module.functions.iter().enumerate() {
        if let FunctionKind::Import {
            module: namespace,
            name,
        } = &f.kind
        {
            let binding = crate::bindings::lookup(f).unwrap();
            let name = crate::bindings::js_name(name);
            if f.import_contract.is_some() {
                let mut offset = 0;
                let mut args = Vec::new();
                for id in &f.params {
                    let ty = &f.locals[id.0].ty;
                    offset = align(offset, adapter.layout(ty).align);
                    args.push(read(ty, &format!("args[0]+{offset}")));
                    offset += adapter.layout(ty).size;
                }
                let convert = if f.result == Type::Unit {
                    "()=>undefined".into()
                } else {
                    format!("(v,args)=>{{{}}}", write(&f.result, "args[1]", "v"))
                };
                out.push_str(&format!(
                    "c.hostImport('host{i}',{}, {},{convert},{},args=>[{}]);\n",
                    quoted(&name),
                    binding.suspends,
                    quoted(namespace),
                    args.join(",")
                ));
            } else {
                let result = match f.result {
                    Type::Value(id) => format!("v=>c.add({},v)", id.0),
                    _ => "v=>v".into(),
                };
                let mut slot = 0;
                let mut decoded = Vec::new();
                for id in &f.params {
                    let ty = &f.locals[id.0].ty;
                    decoded.push(match ty {
                    Type::Num(_) => format!("args[{slot}]"),
                    Type::Value(id) => match adapter.value(*id) {
                        ValueType::Text => format!("new TextDecoder('utf-8',{{fatal:true,ignoreBOM:true}}).decode(new Uint8Array(c.memory.buffer,args[{slot}],args[{}]))",slot+1),
                        ValueType::Bytes => format!("new Uint8Array(c.memory.buffer,args[{slot}],args[{}]).slice()",slot+1),
                        _ => unreachable!("trusted binding parameter"),
                    },
                    _ => unreachable!("trusted binding parameter"),
                });
                    slot += adapter.flat(ty).len();
                }
                out.push_str(&format!(
                    "c.hostImport('host{i}',{}, {},{result},{},args=>[{}]);\n",
                    quoted(&name),
                    binding.suspends,
                    quoted(namespace),
                    decoded.join(",")
                ));
            }
        }
        if f.exports.is_empty() || adapter.module.tests.iter().any(|t| t.function.0 == i) {
            continue;
        }
        let mut offset = 0;
        let mut writes = String::new();
        for (j, id) in f.params.iter().enumerate() {
            let ty = &f.locals[id.0].ty;
            offset = align(offset, adapter.layout(ty).align);
            writes.push_str(&write(ty, &format!("p+{offset}"), &format!("v[{j}]")));
            offset += adapter.layout(ty).size;
        }
        let read = if f.result == Type::Unit {
            "undefined".into()
        } else {
            read(&f.result, "p")
        };
        let post = if f.result == Type::Unit {
            "null".into()
        } else {
            quoted(&format!("__post_{i}"))
        };
        for name in &f.exports {
            let name = crate::bindings::js_name(name);
            out.push_str(&format!("api[{}]=(...v)=>c.call('__export_{i}',{}, {},p=>{{{writes}}},p=>{read},{post},{},v.length);\n", quoted(&name), f.params.len(), offset, effects[i].suspends));
            if effects[i].suspends {
                out.push_str(&format!("asyncExports.push({});\n", quoted(&name)));
            }
        }
    }
    out.push_str(if crate::task_children::enabled(adapter.module) {
        "return {api,asyncExports,childTasks:true};\n}\n"
    } else {
        "return {api,asyncExports};\n}\n"
    });
    out
}
