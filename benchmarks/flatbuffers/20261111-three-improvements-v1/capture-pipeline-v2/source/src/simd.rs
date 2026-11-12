//! Fixed SIMD value instructions. The opcode inventory comes from the pinned
//! wasmparser; signatures and immediates are independently checked by its Wasm
//! validator. Memory and relaxed SIMD stay outside this pure expression family.
use crate::ir::NumType;
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Debug, Serialize)]
pub(crate) struct Signature {
    name: String,
    inputs: Vec<NumType>,
    output: NumType,
    #[serde(skip)]
    immediates: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SimdOp {
    #[serde(flatten)]
    signature: &'static Signature,
    immediates: Vec<String>,
}
impl SimdOp {
    pub fn name(&self) -> &str {
        &self.signature.name
    }
    pub fn inputs(&self) -> &[NumType] {
        &self.signature.inputs
    }
    pub fn output(&self) -> NumType {
        self.signature.output
    }
    pub fn immediates(&self) -> &[String] {
        &self.immediates
    }
    pub(crate) fn wat(&self, args: &str) -> String {
        let immediates = self
            .immediates
            .iter()
            .map(|v| format!(" {v}"))
            .collect::<String>();
        format!("({}{}{args})", self.name(), immediates)
    }
}

fn register(out: &mut Vec<Signature>, proposal: &str, visit: &str, fields: &str, arity: usize) {
    if proposal != "simd" || !matches!(fields, "" | "lane" | "lanes") {
        return;
    }
    let (shape, suffix) = visit
        .strip_prefix("visit_")
        .unwrap()
        .split_once('_')
        .unwrap();
    let name = format!("{shape}.{suffix}");
    let scalar = match shape {
        "i64x2" => NumType::I64,
        "f32x4" => NumType::F32,
        "f64x2" => NumType::F64,
        _ => NumType::I32,
    };
    let (inputs, output) = if suffix == "splat" {
        (vec![scalar], NumType::V128)
    } else if suffix.starts_with("extract_lane") {
        (vec![NumType::V128], scalar)
    } else if suffix == "replace_lane" {
        (vec![NumType::V128, scalar], NumType::V128)
    } else if matches!(suffix, "shl" | "shr_s" | "shr_u") {
        (vec![NumType::V128, NumType::I32], NumType::V128)
    } else if matches!(suffix, "all_true" | "bitmask" | "any_true") {
        (vec![NumType::V128], NumType::I32)
    } else {
        (vec![NumType::V128; arity], NumType::V128)
    };
    out.push(Signature {
        name,
        inputs,
        output,
        immediates: match fields {
            "lane" => 1,
            "lanes" => 16,
            _ => 0,
        },
    });
}

fn catalogue() -> &'static [Signature] {
    static TABLE: OnceLock<Vec<Signature>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut out = Vec::new();
        macro_rules! collect {
            ($( @$proposal:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident (arity $arity:literal -> $results:literal) )*) => {
                $(register(&mut out, stringify!($proposal), stringify!($visit), stringify!($($($arg),*)?), $arity);)*
            };
        }
        wasmparser::for_each_visit_simd_operator!(collect);
        out
    })
}

pub(crate) fn lookup(name: &str) -> Option<&'static Signature> {
    catalogue().iter().find(|s| s.name == name)
}
impl Signature {
    pub(crate) fn immediate_count(&self) -> usize {
        self.immediates
    }
    pub(crate) fn inputs(&self) -> &[NumType] {
        &self.inputs
    }
    pub(crate) fn build(&'static self, immediates: Vec<String>) -> Result<SimdOp, String> {
        if immediates.len() != self.immediates {
            return Err("incorrect SIMD immediate count".into());
        }
        let op = SimdOp {
            signature: self,
            immediates,
        };
        let params = self
            .inputs
            .iter()
            .map(|n| format!(" (param {})", n.wat()))
            .collect::<String>();
        let args = (0..self.inputs.len())
            .map(|i| format!(" (local.get {i})"))
            .collect::<String>();
        validate(&format!(
            "(module (func{params} (result {}) {}))",
            self.output.wat(),
            op.wat(&args)
        ))?;
        Ok(op)
    }
}

fn validate(fragment: &str) -> Result<(), String> {
    let wasm = wat::parse_str(fragment).map_err(|e| format!("invalid SIMD instruction: {e}"))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|e| format!("invalid SIMD instruction: {e}"))?;
    Ok(())
}
pub(crate) fn constant(literal: &str) -> Result<(), String> {
    validate(&format!(
        "(module (func (result v128) (v128.const {literal})))"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_registered_signature_agrees_with_the_wasm_validator() {
        assert!(catalogue().len() > 150);
        for signature in catalogue() {
            let immediates = vec!["0".to_owned(); signature.immediates];
            signature
                .build(immediates)
                .unwrap_or_else(|e| panic!("{}: {e}", signature.name));
        }
        assert!(lookup("v128.load").is_none());
        assert!(lookup("i8x16.relaxed_swizzle").is_none());
    }
}
