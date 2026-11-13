//! Browser-only size reference for the same WAT + validator API as the native benchmark.
//! This fixture does not accept Walz extensions or provide the playground UI API.

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn compile_source(source: &str) -> Result<Vec<u8>, String> {
    let bytes = wat::parse_str(source).map_err(|e| e.to_string())?;
    wasmparser::Validator::new()
        .validate_all(&bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_and_validates_scalar_wat() {
        let source = "(module (func (export \"f\") (result i64) (i64.const 42)))";
        assert_eq!(compile_source(source).unwrap(), wat::parse_str(source).unwrap());
    }

    #[test]
    fn rejects_malformed_and_ill_typed_sources() {
        assert!(compile_source("(").is_err());
        assert!(compile_source("(module (func (result i64) (i32.const 1)))").is_err());
    }
}
