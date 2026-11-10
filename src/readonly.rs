//! Checked loads from a statically bounded, published immutable memory region.
//! Publication and host immutability are explicit trust assumptions.
use crate::{Error, ir::*, sexpr::Sexp};
use serde::{Deserialize, Serialize};

pub const HOST_CONTRACT: &str = "walz-readonly-regions-v1";

#[derive(Clone, Debug, Serialize)]
pub struct RegionDecl {
    pub name: String,
    pub memory: MemoryId,
    pub offset: u32,
    pub length: u32,
    #[serde(skip)]
    pub(crate) span: Span,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub offset: u64,
    pub length: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub region: usize,
    pub opcode: String,
    pub width: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Load {
    pub region: usize,
    pub width: u32,
}

pub(crate) fn opcode(name: &str) -> Option<(&'static str, NumType, u32)> {
    Some(match name {
        "u8" => ("i32.load8_u", NumType::I32, 1),
        "i32" => ("i32.load", NumType::I32, 4),
        "i64" => ("i64.load", NumType::I64, 8),
        "v128" => ("v128.load", NumType::V128, 16),
        _ => return None,
    })
}
pub(crate) fn width(opcode: &str) -> Option<u32> {
    ["u8", "i32", "i64", "v128"]
        .into_iter()
        .filter_map(self::opcode)
        .find(|(name, _, _)| *name == opcode)
        .map(|(_, _, width)| width)
}

pub(crate) fn declaration(
    source: &str,
    node: &Sexp,
    memories: &[MemoryDecl],
) -> Result<RegionDecl, Error> {
    let error = || {
        Error::at(source, node.span,
        "expected (@readonly.region $name $memory byte-offset positive-byte-length), contained in initial memory")
        .with_code("E_READONLY_REGION")
    };
    let Some([_, name, memory, offset, length]) = node.list() else {
        return Err(error());
    };
    let name = name.atom().ok_or_else(error)?.to_owned();
    let memory = memories
        .iter()
        .position(|m| Some(m.name.as_str()) == memory.atom())
        .ok_or_else(error)?;
    let offset = offset
        .atom()
        .and_then(crate::threads::index)
        .ok_or_else(error)?;
    let length = length
        .atom()
        .and_then(crate::threads::index)
        .filter(|n| *n > 0)
        .ok_or_else(error)?;
    if u64::from(offset) + u64::from(length) > u64::from(memories[memory].initial) * 65536 {
        return Err(error());
    }
    Ok(RegionDecl {
        name,
        memory: MemoryId(memory),
        offset,
        length,
        span: node.span,
    })
}

pub(crate) fn helper_name(region: usize, opcode: &str) -> String {
    format!("$__readonly_{region}_{}", opcode.replace('.', "_"))
}
pub(crate) fn helpers(module: &Module) -> String {
    let mut used = std::collections::BTreeSet::new();
    for function in &module.functions {
        let mut pending = function.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            if let ExprKind::Wasm { instruction, .. } = &e.kind
                && let Some(load) = &instruction.readonly
            {
                used.insert((
                    load.region,
                    instruction.opcode.clone(),
                    load.width,
                    e.ty.clone(),
                ));
            }
            pending.extend(e.children());
        }
    }
    used.into_iter().map(|(id, opcode, width, ty)| {
        let region = &module.readonly_regions[id];
        let name = helper_name(id, &opcode);
        let Type::Num(ty) = ty else { unreachable!() };
        if region.length < width {
            return format!("(func {name} (param i32) (result {}) (unreachable))", ty.wat());
        }
        format!("(func {name} (param $offset i32) (result {}) (if (i32.gt_u (local.get $offset) (i32.const {})) (then (unreachable))) ({opcode} {} offset={} (local.get $offset)))",
            ty.wat(), region.length - width, module.memories[region.memory.0].name, region.offset)
    }).collect()
}
