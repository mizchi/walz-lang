//! Reject a valid-but-unrepresentable WIT projection before the pinned decoder
//! registers the same imported/exported type alias twice. Wasm remains valid.
use super::*;
pub(super) fn validate(bytes: &[u8]) -> Result<(), Error> {
    use wasmparser::{Payload, component_types::ComponentEntityType};
    let types = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .map_err(error)?;
    let mut seen = std::collections::HashSet::new();
    let mut depth = 0;
    fn visit(
        t: ComponentEntityType,
        types: &wasmparser::types::Types,
        seen: &mut std::collections::HashSet<wasmparser::component_types::ComponentAnyTypeId>,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > 64 {
            return Err(error("WIT interface nesting limit"));
        }
        match t {
            ComponentEntityType::Type { created, .. } if !seen.insert(created) => {
                return Err(error(
                    "the same instance type alias is both imported and re-exported; this native Component has no supported WIT projection",
                ));
            }
            ComponentEntityType::Instance(id) => {
                for item in types[id].exports.values() {
                    visit(item.ty, types, seen, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    for p in wasmparser::Parser::new(0).parse_all(bytes) {
        match p.map_err(error)? {
            Payload::Version { .. } => depth += 1,
            Payload::End(_) => depth -= 1,
            Payload::ComponentImportSection(s) if depth == 1 => {
                for item in s {
                    let item = item.map_err(error)?;
                    let name = item.name.full_name();
                    if let Some(item) = types.component_item_for_import(&name) {
                        visit(item.ty, &types, &mut seen, 0)?;
                    }
                }
            }
            Payload::ComponentExportSection(s) if depth == 1 => {
                for item in s {
                    let item = item.map_err(error)?;
                    let name = item.name.full_name();
                    if let Some(item) = types.component_item_for_export(&name) {
                        visit(item.ty, &types, &mut seen, 0)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
