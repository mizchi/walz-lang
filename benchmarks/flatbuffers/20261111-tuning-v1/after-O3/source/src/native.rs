//! Module-level signatures for native WAT outside the structured extension IR.

use crate::Error;
use serde::Serialize;
use wasmparser::{CompositeInnerType, ExternalKind, Parser, Payload, TypeRef};

#[derive(Debug, Serialize)]
pub struct NativeTypes {
    pub mode: &'static str,
    pub functions: Vec<FunctionType>,
    /// Stack-form WAT does not have the structured IR's source-expression map.
    pub expression_types: Option<()>,
}

#[derive(Debug, Serialize)]
pub struct FunctionType {
    pub index: u32,
    pub params: Vec<String>,
    pub results: Vec<String>,
    pub exports: Vec<String>,
}

/// Inspect signatures of an already validated core Wasm module.
pub fn types(wasm: &[u8]) -> Result<NativeTypes, Error> {
    let parse = || -> Result<NativeTypes, Box<dyn std::error::Error>> {
        let mut types = Vec::new();
        let mut function_types = Vec::new();
        let mut exports = Vec::new();
        for payload in Parser::new(0).parse_all(wasm) {
            match payload? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        for subtype in group?.into_types() {
                            types.push(match subtype.composite_type.inner {
                                CompositeInnerType::Func(function) => Some(function),
                                _ => None,
                            });
                        }
                    }
                }
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        if let TypeRef::Func(index) | TypeRef::FuncExact(index) = import?.ty {
                            function_types.push(index);
                        }
                    }
                }
                Payload::FunctionSection(reader) => {
                    for index in reader {
                        function_types.push(index?);
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export?;
                        if export.kind == ExternalKind::Func {
                            exports.push((export.index, export.name.to_owned()));
                        }
                    }
                }
                _ => {}
            }
        }
        let functions = function_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                let function = types
                    .get(ty as usize)
                    .and_then(Option::as_ref)
                    .ok_or("function type is unavailable")?;
                Ok(FunctionType {
                    index: index as u32,
                    params: function.params().iter().map(ToString::to_string).collect(),
                    results: function.results().iter().map(ToString::to_string).collect(),
                    exports: exports
                        .iter()
                        .filter(|(function, _)| *function == index as u32)
                        .map(|(_, name)| name.clone())
                        .collect(),
                })
            })
            .collect::<Result<_, Box<dyn std::error::Error>>>()?;
        Ok(NativeTypes {
            mode: "wat",
            functions,
            expression_types: None,
        })
    };
    parse().map_err(|error| Error::message(format!("native Wasm types: {error}")))
}
