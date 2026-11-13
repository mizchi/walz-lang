//! Canonical ABI adapters: private GC values, per-call linear-memory buffers,
//! and Component Model readable handles. JSPI supplies synchronous-ABI waits.
use crate::Error;
use crate::ir::*;
use std::collections::{BTreeMap, BTreeSet};

const MEMORY_LIMIT: u32 = 16 * 1024 * 1024;

mod abi;
pub(crate) mod bound_imports;
pub mod core_js;
mod endpoints;
mod functions;
mod imports;
mod interface;
mod interface_variant;
pub mod js_native;
mod lists;
pub(crate) mod memory;
mod native_delivery;
pub mod profiling;
mod resource_values;
pub(crate) mod stream_batch;
mod values;
mod world;
pub(crate) use world::projection::render as project_interface;

use abi::component_type;

pub struct Adapter<'a> {
    pub(crate) native_resources: bool,
    flat_cache: std::cell::RefCell<std::collections::BTreeMap<Type, Vec<NumType>>>,
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
    if crate::composition::is_component(source) {
        let c = crate::composition::compile(source, policy)?;
        return Ok(Component {
            wasm: c.wasm,
            wat: c.wat,
            manifest: c.manifest,
        });
    }
    let module = checked_module_with_policy(source, policy)?;
    let adapter = Adapter::new(&module);
    adapter.validate_boundary()?;
    let core = crate::emit::emit_component(&module, &adapter);
    let effects = crate::effects::boundary(&module);
    assemble(&module, &adapter, core, effects, None)
}

/// Assemble the original WIT world's Canonical ABI boundary.
/// Supports shared async payloads and typed imported-resource own/borrow handles.
pub fn compile_with_world(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
    path: &std::path::Path,
    name: Option<&str>,
) -> Result<Component, Error> {
    if crate::composition::is_component(source) {
        let c = crate::composition::compile(source, policy)?;
        c.interface.validate_world(path, name)?;
        return Ok(Component {
            wasm: c.wasm,
            wat: c.wat,
            manifest: c.manifest,
        });
    }
    let mut checked = crate::check_with_policy(source, policy)?;
    let roots = world::selected_exports(path, name)?;
    crate::harness::select_application(&mut checked);
    for f in &mut checked.functions {
        f.exports.retain(|n| roots.contains(n));
    }
    let module = prepare_checked_module(source, checked, crate::verified::verify_component, true)?;
    let effects = crate::effects::boundary(&module);
    let adapter = Adapter::new_native(&module);
    adapter.validate_boundary()?;
    if crate::task_children::enabled(&module) {
        return Err(Error::message(
            "native WIT worlds do not yet provide the Walz child scheduler".into(),
        )
        .with_code("E_WIT_WORLD"));
    }
    let world = world::World::load(path, name, &module)?;
    let core = crate::emit::emit_component(&module, &adapter);
    let mut built = assemble(&module, &adapter, core, effects, Some(&world))?;
    built.manifest["nativeWorld"] = true.into();
    Ok(built)
}

/// A composed Walz implementation uses a native value boundary with direct
/// exports. Binding contracts have already passed the ordinary checker gate.
pub(crate) fn compile_composed(
    source: &str,
    module: Module,
    imports: Option<&bound_imports::BoundImports>,
) -> Result<Component, Error> {
    let module = prepare_checked_module(source, module, crate::verified::verify_component, true)?;
    if crate::task_children::enabled(&module) {
        return Err(Error::message(
            "native component composition does not provide the SDK child scheduler".into(),
        )
        .with_code("E_COMPONENT_SOURCE"));
    }
    let effects = crate::effects::boundary(&module);
    let adapter = Adapter::new_native(&module);
    adapter
        .validate_boundary()
        .map_err(|e| e.with_code("E_ABI_TYPE"))?;
    let world = world::World::inferred(&module, imports)?;
    let core = crate::emit::emit_component(&module, &adapter);
    assemble(&module, &adapter, core, effects, Some(&world))
}

fn checked_module(source: &str) -> Result<Module, Error> {
    checked_module_with_policy(source, &crate::bindings::BindingPolicy::default())
}

fn checked_module_with_policy(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
) -> Result<Module, Error> {
    prepare_module(source, policy, crate::verified::verify_component, true)
}

fn checked_browser_module(source: &str, optimize: bool) -> Result<Module, Error> {
    prepare_module(
        source,
        &crate::bindings::BindingPolicy::default(),
        crate::verified::check_browser_component,
        optimize,
    )
}

fn prepare_module(
    source: &str,
    policy: &crate::bindings::BindingPolicy,
    check_contracts: fn(&str, &Module) -> Result<(), Error>,
    optimize: bool,
) -> Result<Module, Error> {
    crate::composition::reject_library_entry(source)?;
    let mut module = crate::check_with_policy(source, policy)?;
    crate::harness::select_application(&mut module);
    prepare_checked_module(source, module, check_contracts, optimize)
}

fn prepare_checked_module(
    source: &str,
    mut module: Module,
    check_contracts: fn(&str, &Module) -> Result<(), Error>,
    optimize: bool,
) -> Result<Module, Error> {
    for f in &module.functions {
        if matches!(f.kind, FunctionKind::Import { .. }) && crate::bindings::lookup(f).is_none() {
            return Err(Error::message(
                "component requires a trusted binding for every import".into(),
            ));
        }
    }
    crate::wit::emit_functions(&module).map_err(|e| e.with_code("E_ABI_TYPE"))?;
    check_contracts(source, &module)?;
    module.abi_suspends = module
        .functions
        .iter()
        .zip(crate::effects::infer(&module))
        .map(|(f, e)| (f.name.clone(), e.suspends))
        .collect();
    let module = if optimize {
        crate::optimize::simplify(&module)
    } else {
        module
    };
    Ok(crate::newtypes::lower(&crate::link::project(&module)))
}

fn assemble(
    module: &Module,
    adapter: &Adapter<'_>,
    core: String,
    effects: Vec<crate::effects::EffectSet>,
    world: Option<&world::World>,
) -> Result<Component, Error> {
    crate::wit::validate_memories(module)?;
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
        let group = world
            .filter(|w| w.has_native_imports())
            .map_or(group, |w| w.import_group_index(namespace, module));
        if let Some(world) = world.filter(|w| w.native_imports_for(namespace)) {
            for (i, f) in functions {
                if let Some(id) = world.close_resource(module, f) {
                    text += &format!(
                        "(core func $host_lower{i} (canon resource.drop $t{}))\n",
                        id.0
                    );
                } else {
                    let FunctionKind::Import { name, .. } = &f.kind else {
                        unreachable!()
                    };
                    text += &format!(
                        "(alias export $host{group} {name:?} (func $host{i}))\n(core func $host_lower{i} (canon lower (func $host{i}) (memory (core memory $mem \"memory\")) (realloc (core func $mem \"realloc\"))))\n"
                    );
                }
            }
            continue;
        }
        let scope = format!("$host{group}_");
        let mut exports = format!("(export \"walz-unit\" (type {scope}unit (eq $unit)))\n");
        for ty in &adapter.types {
            if let Type::Resource(id)
            | Type::Value(id)
            | Type::Newtype(id)
            | Type::Ref { ty: id, .. } = ty
            {
                exports.push_str(&format!(
                    "(export \"t{}\" (type {scope}t{} (eq $t{})))\n",
                    id.0, id.0, id.0
                ));
            }
        }
        if functions.iter().all(|(_, f)| f.import_contract.is_some()) {
            exports = format!("(alias outer 1 $unit (type {scope}unit))\n");
            for ty in &adapter.types {
                if let Type::Resource(id)
                | Type::Value(id)
                | Type::Newtype(id)
                | Type::Ref { ty: id, .. } = ty
                {
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
            let signature = abi::component_signature_named(
                f,
                binding.suspends,
                &scope,
                world.and_then(|w| w.imported_names(namespace, name)),
            );
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
    if crate::task_children::enabled(module) {
        text.push_str(
            "(import \"walz:runtime/tasks@0.1.0\" (instance $children
          (export \"scope-new\" (func (param \"parent\" s32) (result s32)))
          (export \"scope-close\" (func async (param \"id\" s32)))
          (export \"scope-abort\" (func (param \"id\" s32)))
          (export \"child-new\" (func (param \"scope\" s32) (result s32)))
          (export \"child-start\" (func (param \"id\" s32)))
          (export \"child-join\" (func async (param \"id\" s32)))
          (export \"child-cancel\" (func async (param \"id\" s32)))
          (export \"child-timeout\" (func async (param \"id\" s32) (param \"ms\" s32) (result s32)))
          (export \"scope-enter\" (func (param \"id\" s32) (result s32)))
          (export \"child-adopt\" (func (param \"id\" s32) (param \"scope\" s32)))
          (export \"resource-adopt\" (func (param \"id\" s32) (param \"scope\" s32)))))\n",
        );
        for (i, name) in [
            "scope-new",
            "scope-close",
            "scope-abort",
            "child-new",
            "child-start",
            "child-join",
            "child-cancel",
            "child-timeout",
            "scope-enter",
            "child-adopt",
            "resource-adopt",
        ]
        .iter()
        .enumerate()
        {
            text.push_str(&format!("(alias export $children \"{name}\" (func $child_op{i}))\n(core func $child_lower{i} (canon lower (func $child_op{i})))\n"));
            builtin_exports.push_str(&format!("(export \"{name}\" (func $child_lower{i}))\n"));
        }
    }
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
    if crate::task_children::enabled(module) {
        text.push_str("(func $child_run async (param \"id\" s32) (param \"scope\" s32) (canon lift (core func $guest \"__child_run\")))\n(export \"walz-child-run\" (func $child_run))\n");
    }
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
        type_name("walz-unit".into())
    );
    for ty in &adapter.types {
        if let Type::Resource(id) | Type::Value(id) | Type::Newtype(id) | Type::Ref { ty: id, .. } =
            ty
        {
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
    let mut export_functions = BTreeMap::new();
    let (public_types, public_signatures) = world
        .map(|w| w.public_signatures())
        .transpose()?
        .unwrap_or_default();
    text.push_str(&public_types);
    for (i, f) in
        module.functions.iter().enumerate().filter(|(i, f)| {
            !f.exports.is_empty() && !module.tests.iter().any(|t| t.function.0 == *i)
        })
    {
        for (alias, name) in f.exports.iter().enumerate() {
            let async_ = world
                .map(|w| w.is_async(name))
                .unwrap_or(effects[i].suspends);
            let post = if f.result != Type::Unit && adapter.flat(&f.result).len() > 1 {
                format!(" (post-return (core func $guest \"__post_{i}\"))")
            } else {
                String::new()
            };
            text.push_str(&format!("(func $e{i}_{alias} {} (canon lift (core func $guest \"__export_{i}\") (memory (core memory $mem \"memory\")) (realloc (core func $mem \"realloc\")){post}))\n",public_signatures.get(name).cloned().unwrap_or_else(||abi::component_signature_named(f,async_,"$",world.and_then(|w|w.names(name))))));
            let api_signature = abi::component_signature_named(
                f,
                async_,
                "$api_",
                world.and_then(|w| w.names(name)),
            );
            export_functions.insert(name.clone(), format!("$e{i}_{alias}"));
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
            if let Type::Resource(id)
            | Type::Value(id)
            | Type::Newtype(id)
            | Type::Ref { ty: id, .. } = ty
            {
                Some(format!(
                    "(alias export $delivery \"t{}\" (type $t{}))\n",
                    id.0, id.0
                ))
            } else {
                None
            }
        })
        .collect::<String>();

    let (api_exports, outer_exports) = if let Some(world) = world {
        world.render(&export_functions)?
    } else {
        (
            exports,
            "(export \"walz:generated/api@0.1.0\" (instance $api))\n".into(),
        )
    };
    if world.is_some() {
        text.push_str(&api_exports);
        text.push(')');
    } else {
        text.push_str(&format!(
        "(component $api_component\n(import \"types\" (instance $delivery {}))\n(alias export $delivery \"walz-unit\" (type $unit))\n{aliases}{api_types}{api_imports}{api_exports})\n(instance $api (instantiate $api_component (with \"types\" (instance $delivery)){api_arguments}))\n{outer_exports})\n",
        adapter.declarations()
    ));
    }
    let delivery = if world.is_some() {
        format!(
            "(component $types {}{})\n(instance $delivery (instantiate $types{}))\n",
            adapter.materialized_types(),
            native_delivery::body(adapter)?,
            world.map(|w| w.resource_arguments()).unwrap_or_default()
        )
    } else {
        format!(
            "(import \"walz:runtime/delivery@0.1.0\" (instance $delivery {}{factories}))\n",
            adapter.declarations()
        )
    };
    let prefix = world
        .filter(|w| w.has_native_imports())
        .map(|w| w.import_prefix(module))
        .transpose()?
        .unwrap_or_default();
    let text = format!(
        "(component\n{prefix}{delivery}(alias export $delivery \"walz-unit\" (type $unit))\n{aliases}{text}"
    );
    let wasm =
        wat::parse_str(&text).map_err(|e| Error::message(format!("component assembly: {e}")))?;
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
    let mut manifest = serde_json::json!({"version":2,"imports":crate::bindings::manifest(module),"factories":factories,"asyncExports": module.functions.iter().enumerate().filter(|(i,f)|effects[*i].suspends || world.is_some_and(|w|f.exports.iter().any(|n|w.is_async(n)))).flat_map(|(_,f)|f.exports.clone()).collect::<Vec<_>>()});
    if crate::task_children::enabled(module) {
        manifest["childTasks"] = true.into();
    }
    Ok(Component {
        wasm,
        wat: text,
        manifest,
    })
}
