//! Opt-in exact sharing after validation. No source-checking shortcut and no
//! default pipeline change. Unknown custom index metadata is a hard boundary.
use super::inventory;
use crate::Error;
use wasm_encoder::reencode::{self, Reencode};
type Result<T> = std::result::Result<T, reencode::Error<&'static str>>;

struct Sharing {
    indices: Vec<u32>,
    retained: Vec<bool>,
    imports: usize,
}
impl Sharing {
    fn retain(&self, index: u32) -> Result<bool> {
        self.retained
            .get(index as usize)
            .copied()
            .ok_or(reencode::Error::UserError("invalid name index"))
    }
    fn indirect(
        &self,
        maps: wasmparser::IndirectNameMap<'_>,
    ) -> Result<wasm_encoder::IndirectNameMap> {
        let mut output = wasm_encoder::IndirectNameMap::new();
        for map in maps {
            let map = map?;
            if self.retain(map.index)? {
                let mut names = wasm_encoder::NameMap::new();
                for name in map.names {
                    let name = name?;
                    names.append(name.index, name.name);
                }
                output.append(self.indices[map.index as usize], &names);
            }
        }
        Ok(output)
    }
}
impl Reencode for Sharing {
    type Error = &'static str;
    fn function_index(&mut self, index: u32) -> Result<u32> {
        Ok(self.indices[index as usize])
    }
    fn parse_function_section(
        &mut self,
        output: &mut wasm_encoder::FunctionSection,
        input: wasmparser::FunctionSectionReader<'_>,
    ) -> Result<()> {
        for (i, ty) in input.into_iter().enumerate() {
            let ty = ty?;
            if self.retained[self.imports + i] {
                output.function(ty);
            }
        }
        Ok(())
    }
    fn parse_code_section(
        &mut self,
        output: &mut wasm_encoder::CodeSection,
        input: wasmparser::CodeSectionReader<'_>,
    ) -> Result<()> {
        for (i, body) in input.into_iter().enumerate() {
            let body = body?;
            if self.retained[self.imports + i] {
                reencode::utils::parse_function_body(self, output, body)?;
            }
        }
        Ok(())
    }
    fn parse_custom_name_subsection(
        &mut self,
        output: &mut wasm_encoder::NameSection,
        input: wasmparser::Name<'_>,
    ) -> Result<()> {
        match input {
            wasmparser::Name::Function(map) => {
                let mut names = wasm_encoder::NameMap::new();
                for name in map {
                    let name = name?;
                    if self.retain(name.index)? {
                        names.append(self.indices[name.index as usize], name.name);
                    }
                }
                output.functions(&names);
            }
            wasmparser::Name::Local(map) => {
                output.locals(&self.indirect(map)?);
            }
            wasmparser::Name::Label(map) => {
                output.labels(&self.indirect(map)?);
            }
            wasmparser::Name::Unknown { .. } => {
                return Err(reencode::Error::UserError("unknown name subsection"));
            }
            other => reencode::utils::parse_custom_name_subsection(self, output, other)?,
        }
        Ok(())
    }
}

/// Merge only identical private, non-address-taken functions. Source contracts
/// must already have been checked by the producer. Physical Wasm validation is
/// mandatory before and after rewriting, including unreachable instructions.
pub fn share_exact(bytes: &[u8]) -> std::result::Result<Vec<u8>, Error> {
    let report = inventory(bytes)?;
    let count = report.imported_functions + report.functions.len();
    let mut canonical = (0..count as u32).collect::<Vec<_>>();
    for group in &report.groups {
        if let Some((&first, rest)) = group.eligible.split_first() {
            for &index in rest {
                canonical[index as usize] = first;
            }
        }
    }
    let retained = canonical
        .iter()
        .enumerate()
        .map(|(i, &c)| i as u32 == c)
        .collect::<Vec<_>>();
    if retained.iter().all(|&keep| keep) {
        return Ok(bytes.to_vec());
    }
    let rewrite = || -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        for payload in wasmparser::Parser::new(0).parse_all(bytes) {
            if let wasmparser::Payload::CustomSection(section) = payload?
                && section.name() != "name"
            {
                return Err(format!(
                    "custom section `{}` may contain function indices",
                    section.name()
                )
                .into());
            }
        }
        let mut next = 0;
        let positions = retained
            .iter()
            .map(|&keep| {
                let index = next;
                if keep {
                    next += 1;
                }
                index
            })
            .collect::<Vec<u32>>();
        let indices = canonical.iter().map(|&c| positions[c as usize]).collect();
        let mut module = wasm_encoder::Module::new();
        Sharing {
            indices,
            retained,
            imports: report.imported_functions,
        }
        .parse_core_module(&mut module, wasmparser::Parser::new(0), bytes)
        .map_err(|error| format!("{error:?}"))?;
        let result = module.finish();
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&result)?;
        Ok(result)
    };
    rewrite()
        .map_err(|e| Error::message(format!("exact code sharing: {e}")).with_code("E_CODE_SHARING"))
}
