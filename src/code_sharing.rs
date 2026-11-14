//! Read-only inventory of validated Core functions. Candidate equivalence is
//! intentionally strict: physical signature, local encoding and every concrete
//! instruction immediate must match. This never certifies source type identity.
use crate::Error;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use wasmparser::{
    CompositeInnerType, ElementItems, ExternalKind, Operator, Parser, Payload, TypeRef,
};
mod merge;
pub use merge::share_exact;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub schema: &'static str,
    pub module_bytes: usize,
    /// Payload sizes, including the vector count, excluding section id/size.
    pub code_section_bytes: usize,
    pub type_section_bytes: usize,
    pub gc_types: usize,
    pub imported_functions: usize,
    /// Defined functions only; indices retain the original import prefix.
    pub functions: Vec<Function>,
    /// Exact candidates, including protected members for explanatory purposes.
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Function {
    pub index: u32,
    pub name: Option<String>,
    pub type_index: u32,
    pub signature: String,
    pub body_bytes: usize,
    pub instructions: usize,
    pub calls: Vec<u32>,
    pub exports: Vec<String>,
    pub address_taken: bool,
    pub start: bool,
}

#[derive(Debug, Serialize)]
pub struct Group {
    pub members: Vec<u32>,
    /// Private, non-address-taken, non-start members. No merge is performed.
    pub eligible: Vec<u32>,
}

fn references(
    expr: wasmparser::ConstExpr<'_>,
    taken: &mut BTreeSet<u32>,
) -> Result<(), wasmparser::BinaryReaderError> {
    let mut ops = expr.get_operators_reader();
    while !ops.eof() {
        if let Operator::RefFunc { function_index } = ops.read()? {
            taken.insert(function_index);
        }
    }
    Ok(())
}

/// Inspect already validated bytes without changing function/type identity.
/// Invalid binaries and Components are rejected before any candidates escape.
pub fn inventory(bytes: &[u8]) -> Result<Inventory, Error> {
    let inspect = || -> Result<Inventory, Box<dyn std::error::Error>> {
        if !Parser::is_core_wasm(bytes) {
            return Err("expected a Core Wasm module".into());
        }
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(bytes)?;
        let mut report = Inventory {
            schema: "walz-code-inventory-v1",
            module_bytes: bytes.len(),
            code_section_bytes: 0,
            type_section_bytes: 0,
            gc_types: 0,
            imported_functions: 0,
            functions: vec![],
            groups: vec![],
        };
        let mut types = vec![];
        let mut indices = vec![];
        let mut names = BTreeMap::new();
        let mut exports: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        let mut taken = BTreeSet::new();
        let mut start = None;
        let mut keys: BTreeMap<(String, Vec<u8>), Vec<u32>> = BTreeMap::new();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload? {
                Payload::TypeSection(reader) => {
                    let range = reader.range();
                    report.type_section_bytes = (range.end - range.start) as usize;
                    for group in reader {
                        for ty in group?.into_types() {
                            let signature = format!("{ty:?}");
                            types.push(match ty.composite_type.inner {
                                CompositeInnerType::Func(_) => Some(signature),
                                CompositeInnerType::Struct(_) | CompositeInnerType::Array(_) => {
                                    report.gc_types += 1;
                                    None
                                }
                                _ => None,
                            });
                        }
                    }
                }
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        if matches!(import?.ty, TypeRef::Func(_) | TypeRef::FuncExact(_)) {
                            report.imported_functions += 1;
                        }
                    }
                }
                Payload::FunctionSection(reader) => {
                    indices = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let e = export?;
                        if matches!(e.kind, ExternalKind::Func | ExternalKind::FuncExact) {
                            exports.entry(e.index).or_default().push(e.name.into());
                        }
                    }
                }
                Payload::StartSection { func, .. } => start = Some(func),
                Payload::GlobalSection(reader) => {
                    for global in reader {
                        references(global?.init_expr, &mut taken)?;
                    }
                }
                Payload::TableSection(reader) => {
                    for table in reader {
                        if let wasmparser::TableInit::Expr(expr) = table?.init {
                            references(expr, &mut taken)?;
                        }
                    }
                }
                Payload::ElementSection(reader) => {
                    for element in reader {
                        let element = element?;
                        if let wasmparser::ElementKind::Active { offset_expr, .. } = element.kind {
                            references(offset_expr, &mut taken)?;
                        }
                        match element.items {
                            ElementItems::Functions(items) => {
                                for index in items {
                                    taken.insert(index?);
                                }
                            }
                            ElementItems::Expressions(_, items) => {
                                for expr in items {
                                    references(expr?, &mut taken)?;
                                }
                            }
                        }
                    }
                }
                Payload::DataSection(reader) => {
                    for data in reader {
                        if let wasmparser::DataKind::Active { offset_expr, .. } = data?.kind {
                            references(offset_expr, &mut taken)?;
                        }
                    }
                }
                Payload::CodeSectionStart {
                    unchecked_range: range,
                    ..
                } => report.code_section_bytes = (range.end - range.start) as usize,
                Payload::CodeSectionEntry(body) => {
                    let ordinal = report.functions.len();
                    let index = (report.imported_functions + ordinal) as u32;
                    let type_index = indices[ordinal];
                    let signature = types[type_index as usize]
                        .as_ref()
                        .ok_or("missing function type")?
                        .clone();
                    let mut ops = body.get_operators_reader()?;
                    let mut instructions = 0;
                    let mut calls = BTreeSet::new();
                    while !ops.eof() {
                        instructions += 1;
                        match ops.read()? {
                            Operator::Call { function_index }
                            | Operator::ReturnCall { function_index } => {
                                calls.insert(function_index);
                            }
                            Operator::RefFunc { function_index } => {
                                taken.insert(function_index);
                            }
                            _ => (),
                        }
                    }
                    let body = body.as_bytes();
                    keys.entry((signature.clone(), body.to_vec()))
                        .or_default()
                        .push(index);
                    report.functions.push(Function {
                        index,
                        name: None,
                        type_index,
                        signature,
                        body_bytes: body.len(),
                        instructions,
                        calls: calls.into_iter().collect(),
                        exports: vec![],
                        address_taken: false,
                        start: false,
                    });
                }
                Payload::CustomSection(section) => {
                    if let wasmparser::KnownCustom::Name(reader) = section.as_known() {
                        for name in reader {
                            if let wasmparser::Name::Function(map) = name? {
                                for name in map {
                                    let name = name?;
                                    names.insert(name.index, name.name.to_owned());
                                }
                            }
                        }
                    }
                }
                _ => (),
            }
        }
        for f in &mut report.functions {
            f.name = names.remove(&f.index);
            f.exports = exports.remove(&f.index).unwrap_or_default();
            f.address_taken = taken.contains(&f.index);
            f.start = start == Some(f.index);
        }
        report.groups = keys
            .into_values()
            .filter(|members| members.len() > 1)
            .map(|members| {
                let eligible = members
                    .iter()
                    .copied()
                    .filter(|&index| {
                        let f = &report.functions[index as usize - report.imported_functions];
                        f.exports.is_empty() && !f.address_taken && !f.start
                    })
                    .collect();
                Group { members, eligible }
            })
            .collect();
        report.groups.sort_by_key(|g| g.members[0]);
        Ok(report)
    };
    inspect()
        .map_err(|e| Error::message(format!("code inventory: {e}")).with_code("E_CODE_SHARING"))
}
