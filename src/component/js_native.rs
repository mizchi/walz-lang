//! Experimental synchronous JS ABI. Shared GC values stay in the guest; only
//! checked, generated constructors/accessors cross the private host boundary.
//! This is not a Component ABI and intentionally has no host capabilities,
//! owned endpoints, shared memory, cancellation API or persistent handle table.
use super::{Adapter, abi::type_key};
use crate::{Error, ir::*};
use std::collections::BTreeSet;

mod strings;
mod values;
pub(crate) use strings::from_utf8 as string_from_utf8;
pub(crate) use strings::{decode as string_decode, literals};
pub(crate) use strings::{helpers as string_helpers, imports as string_imports};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Utf8,
    JsStrings,
}

pub struct NativeJs {
    pub wat: String,
    pub wasm: Vec<u8>,
    pub bindings: String,
    pub manifest: serde_json::Value,
}
fn unsupported(message: &str) -> Error {
    Error::message(format!("experimental JS native ABI: {message}"))
        .with_code("E_JS_NATIVE_UNSUPPORTED")
}

pub fn compile(source: &str, mode: Mode) -> Result<NativeJs, Error> {
    // Reuse the full type/effect/ownership/law gate before selecting a backend.
    let mut module =
        super::checked_module_with_policy(source, &crate::bindings::BindingPolicy::default())?;
    if !module.memories.is_empty()
        || crate::task_children::enabled(&module)
        || module
            .functions
            .iter()
            .any(|f| matches!(f.kind, FunctionKind::Import { .. }))
        || module.types.iter().any(|t| {
            matches!(
                t.kind,
                TypeKind::Resource { .. }
                    | TypeKind::Value(
                        ValueType::Own(_)
                            | ValueType::Future(_)
                            | ValueType::Stream(_)
                            | ValueType::Task(_)
                    )
            )
        })
        || crate::effects::boundary(&module).iter().any(|e| e.suspends)
    {
        return Err(unsupported(
            "requires synchronous shared values without host imports, owners or source memories",
        ));
    }
    let adapter = Adapter::new(&module);
    adapter.validate_boundary()?;
    let mut types = BTreeSet::new();
    for f in module.functions.iter().filter(|f| !f.exports.is_empty()) {
        for id in &f.params {
            super::core_js::collect(&adapter, &f.locals[id.0].ty, &mut types);
        }
        super::core_js::collect(&adapter, &f.result, &mut types);
    }
    for ty in &types {
        values::validate(&adapter, ty)?;
    }
    let mut generator = values::Generator::new(&adapter, mode);
    for ty in &types {
        generator.codec(ty)?;
    }
    let bridges = generator.wat;
    let mut bindings = format!(
        "// Experimental generated direct GC boundary.\nexport function bind(x,c){{\n{}\nconst api=Object.create(null);\n",
        generator.js
    );
    for (i, f) in module
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.exports.is_empty())
    {
        let conversions = f
            .params
            .iter()
            .enumerate()
            .map(|(j, id)| format!("const a{j}=w_{}(v[{j}]);", type_key(&f.locals[id.0].ty)))
            .collect::<String>();
        let args = (0..f.params.len())
            .map(|j| format!("a{j}"))
            .collect::<Vec<_>>()
            .join(",");
        let call = format!("c.invoke(()=>x.__call_{i}({args}))");
        let read = if matches!(f.result, Type::Unit | Type::Never) {
            format!("{call};return undefined;")
        } else {
            format!("return r_{}({call});", type_key(&f.result))
        };
        for name in &f.exports {
            bindings += &format!(
                "api[{}]=(...v)=>c.run(()=>{{if(v.length!=={})throw new TypeError('argument count');{conversions}{read}}});\n",
                serde_json::to_string(&crate::bindings::js_name(name)).unwrap(),
                f.params.len()
            );
        }
    }
    bindings += "return api;\n}\n";
    drop(adapter);
    // Public exports belong to the generated JS API; keep raw GC access private.
    for (i, f) in module.functions.iter_mut().enumerate() {
        if !f.exports.is_empty() {
            f.exports = vec![format!("__call_{i}")];
        }
    }
    let mut wat = if mode == Mode::JsStrings {
        crate::emit::emit_js_strings(&module)
    } else {
        crate::emit::emit(&module)
    };
    wat.truncate(wat.rfind(')').unwrap());
    if !module
        .types
        .iter()
        .any(|t| matches!(t.kind, TypeKind::Value(ValueType::Bytes | ValueType::Text)))
    {
        wat += "(type $__byte_storage (array (mut i8)))\n";
    }
    wat += strings::transfer_helpers();
    wat += &bridges;
    wat += ")\n";
    let wasm = wat::parse_str(&wat).map_err(|e| Error::message(format!("JS native WAT: {e}")))?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .map_err(|e| Error::message(format!("JS native validation: {e}")))?;
    Ok(NativeJs {
        wat,
        wasm,
        bindings,
        manifest: serde_json::json!({
            "abi":"walz-js-native-experiment-v1", "jsStrings":mode==Mode::JsStrings,
            "synchronous":true,"maximumTransferBytes":super::MEMORY_LIMIT,
            "note":"Experimental shared-value JS ABI, not Canonical ABI; no start/cancellation/host imports."
        }),
    })
}
