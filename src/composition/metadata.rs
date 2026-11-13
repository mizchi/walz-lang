//! Optional Core/JS FFI descriptions. Metadata never grants capabilities or
//! changes a Wasm type. Portable Component functions still require WIT values.
use crate::Error;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ffi {
    version: u32,
    functions: Vec<Function>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Function {
    export: String,
    params: Vec<String>,
    results: Vec<String>,
}
fn fail(e: impl std::fmt::Display) -> Error {
    Error::message(format!("walz:ffi-v1: {e}")).with_code("E_FFI_METADATA")
}

pub(crate) fn validate(bytes: &[u8]) -> Result<(), Error> {
    if wasmparser::Parser::is_core_wasm(bytes) {
        return core(bytes);
    }
    let mut stack = vec![];
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload.map_err(fail)? {
            wasmparser::Payload::Version { encoding, .. } => stack.push(encoding),
            wasmparser::Payload::End(_) => {
                stack.pop();
            }
            wasmparser::Payload::ModuleSection {
                unchecked_range, ..
            } => core(&bytes[unchecked_range.start as usize..unchecked_range.end as usize])?,
            wasmparser::Payload::CustomSection(s)
                if s.name() == "walz:ffi-v1"
                    && stack.last() == Some(&wasmparser::Encoding::Component) =>
            {
                return Err(fail(
                    "attach Core signature metadata to a core module, not a component",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
fn core(bytes: &[u8]) -> Result<(), Error> {
    let mut metadata = None;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CustomSection(s) = payload.map_err(fail)? {
            if s.name() != "walz:ffi-v1" {
                continue;
            }
            if metadata.is_some() {
                return Err(fail("duplicate metadata section"));
            }
            metadata = Some(serde_json::from_slice::<Ffi>(s.data()).map_err(fail)?);
        }
    }
    let Some(ffi) = metadata else {
        return Ok(());
    };
    if ffi.version != 1 {
        return Err(fail("unsupported schema version"));
    }
    let types = crate::native::types(bytes)?;
    let mut seen = std::collections::BTreeSet::new();
    for f in ffi.functions {
        if !seen.insert(f.export.clone()) {
            return Err(fail("duplicate function description"));
        }
        for t in f.params.iter().chain(&f.results) {
            if !matches!(
                t.as_str(),
                "i32" | "i64" | "f32" | "f64" | "v128" | "externref" | "funcref"
            ) {
                return Err(fail(format!("unsupported Core value type {t}")));
            }
        }
        let actual = types
            .functions
            .iter()
            .find(|a| a.exports.contains(&f.export))
            .ok_or_else(|| fail(format!("missing Core function export {}", f.export)))?;
        if actual.params != f.params || actual.results != f.results {
            return Err(fail(format!(
                "signature differs for Core export {}",
                f.export
            )));
        }
    }
    Ok(())
}
