//! An executable dependency is a definition, not an implicit instance. Binary
//! replacement is indexed by explicit constructor declarations; custom-section
//! bytes are never interpreted as executable source.
use super::*;
pub(crate) const MAX_BINARY: usize = 4 * 1024 * 1024;
pub(super) type Registry = BTreeMap<usize, Vec<u8>>;
pub(crate) fn declaration(name: &str, bytes: &[u8]) -> Result<String, Error> {
    validate(bytes)?;
    Ok(format!(
        "(@component.constructor {name} {})",
        crate::binary::quoted(bytes)
    ))
}
pub(super) fn validate(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > MAX_BINARY {
        return Err(error("component dependency exceeds 4 MiB"));
    }
    if !wasmparser::Parser::is_component(bytes) {
        return Err(error("a component constructor requires Component Wasm"));
    }
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .map_err(error)?;
    Ok(())
}
pub(super) fn resolve(bytes: &[u8], embedded: &Registry) -> Result<Vec<u8>, Error> {
    if embedded.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut result = wasm_encoder::Component::new();
    let mut depth = 0;
    let mut ordinal = 0;
    let mut seen = 0;
    for p in wasmparser::Parser::new(0).parse_all(bytes) {
        let p = p.map_err(error)?;
        if depth == 1
            && let Some((id, range)) = p.as_section()
        {
            let original = &bytes[range.start as usize..range.end as usize];
            let mut data = original;
            if matches!(p, wasmparser::Payload::ComponentSection { .. }) {
                if let Some(bytes) = embedded.get(&ordinal) {
                    data = bytes;
                    seen += 1;
                }
                ordinal += 1;
            }
            result.section(&wasm_encoder::RawSection { id, data });
        }
        match p {
            wasmparser::Payload::Version { .. } => depth += 1,
            wasmparser::Payload::End(_) => depth -= 1,
            _ => {}
        }
    }
    if seen != embedded.len() {
        return Err(error("missing binary constructor definition"));
    }
    Ok(result.finish())
}
