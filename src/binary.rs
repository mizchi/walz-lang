//! Typed Component dependencies. WIT supplies values, never purity or ownership
//! of arbitrary guest code. Binary contents are snapshotted by the source loader.
use crate::{
    Error,
    ir::*,
    sexpr::{Kind, Sexp},
};

pub(crate) const PREFIX: &str = "walz:component/";
pub(crate) const MAX_BINARY: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Export {
    pub selector: String,
    pub path: Vec<String>,
    pub binding: crate::bindings::Binding,
    #[serde(skip)]
    pub(crate) declaration: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Dependency {
    pub namespace: String,
    pub exports: Vec<Export>,
    #[serde(skip)]
    pub bytes: std::sync::Arc<[u8]>,
}

pub(crate) struct Unavailable {
    pub selector: String,
    pub reason: String,
}
pub(crate) struct Interface {
    pub source: String,
    pub exports: Vec<Export>,
    pub unavailable: Vec<Unavailable>,
}

fn failure(message: impl std::fmt::Display) -> Error {
    Error::message(format!("Wasm @use: {message}")).with_code("E_WASM_USE")
}

fn inspect(bytes: &[u8]) -> Result<Interface, Error> {
    if bytes.len() > MAX_BINARY {
        return Err(failure("binary exceeds 4 MiB"));
    }
    if !wasmparser::Parser::is_component(bytes) {
        return Err(failure(
            "expected a WIT-typed Component Wasm; raw Core Wasm needs a Canonical ABI component wrapper",
        ));
    }
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .map_err(failure)?;
    let wit_parser::decoding::DecodedWasm::Component(resolve, world) =
        wit_parser::decoding::decode(bytes).map_err(failure)?
    else {
        return Err(failure(
            "a WIT package describes types but is not an executable component",
        ));
    };
    if !resolve.worlds[world].imports.is_empty() {
        return Err(failure(
            "component dependencies must be closed: imported host capabilities need explicit composition first",
        ));
    }
    crate::wit_bindgen::component_exports(&resolve, world)
        .map_err(|e| failure(e.diagnostic().message.clone()))
}

pub(crate) fn source(bytes: &[u8], identity: &str) -> Result<(String, Vec<Unavailable>), Error> {
    let interface = inspect(bytes)?;
    let namespace = format!("{PREFIX}{identity}");
    let mut text = format!(
        "(@component.binary {namespace:?} {})\n{}",
        quoted(bytes),
        interface.source
    );
    for (i, export) in interface.exports.iter().enumerate() {
        text.push_str(&format!(
            "(import {namespace:?} \"export-{i}\" (func {}{}))\n(@export (func {}))\n",
            export.selector, export.declaration, export.selector
        ));
    }
    Ok((text, interface.unavailable))
}

pub(crate) fn declaration(source: &str, node: &Sexp) -> Result<Dependency, Error> {
    let err = || {
        Error::at(
            source,
            node.span,
            "expected embedded Component Wasm descriptor",
        )
        .with_code("E_WASM_USE")
    };
    let Some([_, namespace, data]) = node.list() else {
        return Err(err());
    };
    let namespace = namespace
        .text()
        .filter(|s| s.starts_with(PREFIX))
        .ok_or_else(err)?;
    let bytes = match &data.kind {
        Kind::Bytes(b) => b.as_slice(),
        Kind::Text(s) => s.as_bytes(),
        _ => return Err(err()),
    };
    let interface = inspect(bytes).map_err(|e| {
        Error::at(source, node.span, &e.diagnostic().message).with_code("E_WASM_USE")
    })?;
    Ok(Dependency {
        namespace: namespace.into(),
        exports: interface.exports,
        bytes: bytes.into(),
    })
}

pub(crate) fn apply(module: &mut Module) -> Result<(), Error> {
    let mut seen = std::collections::BTreeSet::new();
    for dependency in &module.components {
        if !seen.insert(&dependency.namespace) {
            return Err(failure("duplicate component dependency identity"));
        }
    }
    for function in &mut module.functions {
        let FunctionKind::Import {
            module: namespace,
            name,
        } = &function.kind
        else {
            continue;
        };
        if !namespace.starts_with(PREFIX) {
            continue;
        }
        let dependency = module
            .components
            .iter()
            .find(|d| &d.namespace == namespace)
            .ok_or_else(|| failure("missing embedded component"))?;
        let export = name
            .strip_prefix("export-")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|i| dependency.exports.get(i))
            .ok_or_else(|| failure("unknown component export"))?;
        function.import_contract = Some(export.binding.clone());
    }
    Ok(())
}

pub(crate) fn core(module: &Module) -> Result<(), Error> {
    if module.components.is_empty() {
        Ok(())
    } else {
        Err(Error::message("@use of Component Wasm requires --target component; Core and Core+JSPI cannot instantiate components".into()).with_code("E_COMPONENT_USE_BACKEND"))
    }
}

pub(crate) fn quoted(bytes: &[u8]) -> String {
    format!(
        "\"{}\"",
        bytes
            .iter()
            .map(|b| format!("\\{b:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn expanded_wit_graphs_are_bounded_before_building_shape_strings() {
        let mut wit =
            String::from("package test:limits; interface api { record t0 { value: s32 } ");
        for i in 1..=12 {
            wit += &format!("record t{i} {{ a: t{}, b: t{} }} ", i - 1, i - 1);
        }
        wit += "heavy: func(value: t12); small: func() -> s32; } world guest { export api; }";
        let mut resolve = wit_parser::Resolve::default();
        let package = resolve.push_str("api.wit", &wit).unwrap();
        let world = resolve.select_world(&[package], Some("guest")).unwrap();
        let interface = crate::wit_bindgen::component_exports(&resolve, world).unwrap();
        assert!(interface.exports.iter().any(|e| e.selector == "$api.small"));
        assert!(
            interface
                .unavailable
                .iter()
                .any(|e| e.selector == "$api.heavy" && e.reason.contains("limit"))
        );
    }
}
