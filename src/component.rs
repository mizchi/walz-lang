//! Canonical ABI adapters: private GC values, per-call linear-memory buffers,
//! and Component Model readable handles. JSPI supplies synchronous-ABI waits.
use crate::Error;
use crate::ir::*;
use std::collections::{BTreeMap, BTreeSet};

const MEMORY_LIMIT: u32 = 16 * 1024 * 1024;

mod abi;
pub mod core_js;
mod endpoints;
mod functions;
mod imports;
mod lists;
pub mod profiling;
mod values;

use abi::{component_signature, component_type};

pub struct Adapter<'a> {
    pub module: &'a Module,
    pub endpoints: BTreeSet<TypeId>,
    types: BTreeSet<Type>,
    memory_exports: bool,
}

pub struct Component {
    pub wasm: Vec<u8>,
    pub wat: String,
    pub manifest: serde_json::Value,
}

pub fn compile(source: &str) -> Result<Component, Error> {
    compile_with_policy(source, &crate::bindings::BindingPolicy::default())
}

pub fn compile_with_policy(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
) -> Result<Component, Error> {
    let module = checked_module_with_policy(source, policy)?;
    let adapter = Adapter::new(&module);
    let core = crate::emit::emit_component(&module, &adapter);
    let effects = crate::effects::boundary(&module);
    assemble(&module, &adapter, core, effects)
}

fn checked_module(source: &str) -> Result<Module, Error> {
    checked_module_with_policy(source, &crate::bindings::BindingPolicy::default())
}

fn checked_module_with_policy(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
) -> Result<Module, Error> {
    let mut module = crate::check_with_policy(source, policy)?;
    for f in &module.functions {
        if matches!(f.kind, FunctionKind::Import { .. }) && crate::bindings::lookup(f).is_none() {
            return Err(Error::message(
                "component requires a trusted binding for every import".into(),
            ));
        }
    }
    crate::wit::emit(&module)?;
    crate::verified::verify_component(source, &module)?;
    module.abi_suspends = module
        .functions
        .iter()
        .zip(crate::effects::infer(&module))
        .map(|(f, e)| (f.name.clone(), e.suspends))
        .collect();
    Ok(crate::link::project(&crate::optimize::simplify(&module)))
}

fn assemble(
    module: &Module,
    adapter: &Adapter<'_>,
    core: String,
    effects: Vec<crate::effects::EffectSet>,
) -> Result<Component, Error> {
    let mut text = format!(
        "(core module $mem {})\n(core instance $mem (instantiate $mem))\n",
        include_str!("component-memory.wat")
    );
    let mut groups = BTreeMap::<&str, Vec<(usize, &Function)>>::new();
    for (i, function) in module.functions.iter().enumerate() {
        if let FunctionKind::Import { module, .. } = &function.kind {
            groups.entry(module).or_default().push((i, function));
        }
    }
    for (group, (namespace, functions)) in groups.iter().enumerate() {
        let scope = format!("$host{group}_");
        let mut exports = format!("(export \"wlt-unit\" (type {scope}unit (eq $unit)))\n");
        for ty in &adapter.types {
            if let Type::Value(id) = ty {
                exports.push_str(&format!(
                    "(export \"t{}\" (type {scope}t{} (eq $t{})))\n",
                    id.0, id.0, id.0
                ));
            }
        }
        if functions.iter().all(|(_, f)| f.import_contract.is_some()) {
            exports = format!("(alias outer 1 $unit (type {scope}unit))\n");
            for ty in &adapter.types {
                if let Type::Value(id) = ty {
                    exports.push_str(&format!(
                        "(alias outer 1 $t{} (type {scope}t{}))\n",
                        id.0, id.0
                    ));
                }
            }
        }
        let mut lowers = String::new();
        for (i, f) in functions {
            let FunctionKind::Import { name, .. } = &f.kind else {
                unreachable!()
            };
            let binding = crate::bindings::lookup(f).unwrap();
            let signature = component_signature(f, binding.suspends, &scope);
            exports.push_str(&format!("(export \"{name}\" (func {signature}))\n"));
            // Both directions copy UTF-8 strings through the same arena.
            lowers.push_str(&format!("(alias export $host{group} \"{name}\" (func $host{i}))\n(core func $host_lower{i} (canon lower (func $host{i}) (memory (core memory $mem \"memory\")) (realloc (core func $mem \"realloc\"))))\n"));
        }
        text.push_str(&format!(
            "(import \"{namespace}\" (instance $host{group} {exports}))\n{lowers}"
        ));
    }
    let mut factories = String::new();
    let mut factory_lowers = String::new();
    let mut builtin_exports = String::new();
    for id in &adapter.endpoints {
        let n = id.0;
        let payload = match adapter.value(*id) {
            ValueType::Future(t) | ValueType::Stream(t) => t,
            _ => unreachable!(),
        };
        let future = matches!(adapter.value(*id), ValueType::Future(_));
        let kind = if future { "future" } else { "stream" };
        factories.push_str(&format!(
            "(type $list{n} (list {}))\n",
            component_type(payload)
        ));
        text.push_str(&format!("(core func $read{n} (canon {kind}.read $t{n} (memory (core memory $mem \"memory\")) (realloc (core func $mem \"realloc\"))))\n(core func $drop{n} (canon {kind}.drop-readable $t{n}))\n"));
        factories.push_str(&format!(
            "(export \"ready-t{n}\" (func (param \"items\" $list{n}) (result $t{n})))\n"
        ));
        factory_lowers.push_str(&format!("(alias export $delivery \"ready-t{n}\" (func $ready_fn{n}))\n(core func $ready{n} (canon lower (func $ready_fn{n}) (memory (core memory $mem \"memory\"))))\n"));
        builtin_exports.push_str(&format!("(export \"read{n}\" (func $read{n})) (export \"drop{n}\" (func $drop{n})) (export \"ready{n}\" (func $ready{n}))\n"));
        if !future {
            factories.push_str(&format!("(export \"prepend-t{n}\" (func (param \"items\" $list{n}) (param \"tail\" $t{n}) (result $t{n})))\n"));
            factory_lowers.push_str(&format!("(alias export $delivery \"prepend-t{n}\" (func $prepend_fn{n}))\n(core func $prepend{n} (canon lower (func $prepend_fn{n}) (memory (core memory $mem \"memory\"))))\n"));
            builtin_exports.push_str(&format!("(export \"prepend{n}\" (func $prepend{n}))\n"));
        }
    }
    text.push_str(&factory_lowers);
    let core = core
        .strip_prefix("(module")
        .unwrap()
        .trim_end()
        .strip_suffix(')')
        .unwrap();
    text.push_str(&format!("(core module $guest {core})\n(core instance $env (export \"memory\" (memory $mem \"memory\")) (export \"alloc\" (func $mem \"alloc\")) (export \"free\" (func $mem \"free\")) (export \"free-all\" (func $mem \"free-all\")) (export \"cancel-requested\" (global $mem \"cancel-requested\")) {builtin_exports}"));
    for (i, _) in module
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| matches!(f.kind, FunctionKind::Import { .. }))
    {
        text.push_str(&format!("(export \"host{i}\" (func $host_lower{i}))"));
    }
    text.push_str(
        ")\n(core instance $guest (instantiate $guest (with \"__component\" (instance $env))))\n",
    );
    let used = module
        .functions
        .iter()
        .flat_map(|f| f.exports.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let type_name = |mut name: String| {
        while used.contains(&name) {
            name.push_str("-type");
        }
        name
    };
    let mut api_types = format!(
        "(export $api_unit \"{}\" (type $unit))\n",
        type_name("wlt-unit".into())
    );
    for ty in &adapter.types {
        if let Type::Value(id) = ty {
            api_types.push_str(&format!(
                "(export $api_t{} \"{}\" (type $t{}))\n",
                id.0,
                type_name(format!("t{}", id.0)),
                id.0
            ));
        }
    }
    let mut api_imports = String::new();
    let mut api_arguments = String::new();
    let mut exports = String::new();
    for (i, f) in
        module.functions.iter().enumerate().filter(|(i, f)| {
            !f.exports.is_empty() && !module.tests.iter().any(|t| t.function.0 == *i)
        })
    {
        for (alias, name) in f.exports.iter().enumerate() {
            let post = if f.result != Type::Unit && adapter.flat(&f.result).len() > 1 {
                format!(" (post-return (core func $guest \"__post_{i}\"))")
            } else {
                String::new()
            };
            text.push_str(&format!("(func $e{i}_{alias} {} (canon lift (core func $guest \"__export_{i}\") (memory (core memory $mem \"memory\")) (realloc (core func $mem \"realloc\")){post}))\n",component_signature(f,effects[i].suspends,"$")));
            let api_signature = component_signature(f, effects[i].suspends, "$api_");
            api_imports.push_str(&format!(
                "(import \"f{i}-{alias}\" (func $e{i}_{alias} {api_signature}))\n"
            ));
            api_arguments.push_str(&format!(" (with \"f{i}-{alias}\" (func $e{i}_{alias}))"));
            exports.push_str(&format!("(export \"{name}\" (func $e{i}_{alias}))\n"));
        }
    }
    let aliases = adapter
        .types
        .iter()
        .filter_map(|ty| {
            if let Type::Value(id) = ty {
                Some(format!(
                    "(alias export $delivery \"t{}\" (type $t{}))\n",
                    id.0, id.0
                ))
            } else {
                None
            }
        })
        .collect::<String>();
    text.push_str(&format!(
        "(component $api_component\n(import \"types\" (instance $delivery {}))\n(alias export $delivery \"wlt-unit\" (type $unit))\n{aliases}{api_types}{api_imports}{exports})\n(instance $api (instantiate $api_component (with \"types\" (instance $delivery)){api_arguments}))\n(export \"wlt:generated/api@0.1.0\" (instance $api)))\n",
        adapter.declarations()
    ));
    let text = format!(
        "(component\n(import \"wlt:runtime/delivery@0.1.0\" (instance $delivery {}{factories}))\n(alias export $delivery \"wlt-unit\" (type $unit))\n{aliases}{text}",
        adapter.declarations()
    );
    let wasm = wat::parse_str(&text).map_err(|e| Error::message(format!("component WAT: {e}")))?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .map_err(|e| Error::message(format!("component validation: {e}")))?;
    let factories = adapter
        .endpoints
        .iter()
        .map(|id| {
            let (kind, payload) = match adapter.value(*id) {
                ValueType::Future(t) => ("future", t),
                ValueType::Stream(t) => ("stream", t),
                _ => unreachable!(),
            };
            serde_json::json!({"id":id.0,"kind":kind,"unit":*payload==Type::Unit})
        })
        .collect::<Vec<_>>();
    Ok(Component {
        wasm,
        wat: text,
        manifest: serde_json::json!({"version":2,"imports":crate::bindings::manifest(module),"factories":factories,"asyncExports": module.functions.iter().enumerate().filter(|(i,_)|effects[*i].suspends).flat_map(|(_,f)|f.exports.clone()).collect::<Vec<_>>()}),
    })
}
