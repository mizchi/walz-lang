//! Private JSPI ABI. The guest shares the checked GC lowering and cancellation
//! protocol with Components; JS owns only boundary buffers and external ends.
use super::Adapter;
use super::abi::{align, type_key};
use crate::Error;
use crate::ir::*;
use std::collections::BTreeSet;

mod values;
use values::{codec, read, write};

pub struct CoreJs {
    pub wasm: Vec<u8>,
    pub memory: Vec<u8>,
    pub bindings: String,
    pub manifest: serde_json::Value,
}

pub fn compile(source: &str) -> Result<CoreJs, Error> {
    compile_with_policy(source, &crate::bindings::BindingPolicy::default())
}

pub fn compile_with_policy(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
) -> Result<CoreJs, Error> {
    let module = super::checked_module_with_policy(source, policy)?;
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
    let wasm = parse(&crate::emit::emit_component(&module, &adapter))?;
    let memory = parse(&format!(
        "(module {})",
        include_str!("../component-memory.wat")
    ))?;
    let bindings = bindings(&adapter);
    Ok(CoreJs {
        wasm,
        memory,
        bindings,
        manifest: serde_json::json!({"abi":"wlt-core-js-v2","imports":crate::bindings::manifest(&module)}),
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
    if let Type::Value(id) = ty {
        match adapter.value(*id) {
            ValueType::List(t)
            | ValueType::Option(t)
            | ValueType::Future(t)
            | ValueType::Stream(t) => collect(adapter, t, used),
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
    out.push_str("return {api,asyncExports};\n}\n");
    out
}
