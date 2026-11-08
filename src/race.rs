//! Fixed atomic-cell certificates. The kernel proves an unbounded trace property
//! under a host contract; extraction, lowering, the engine, and host compliance
//! are trusted. This is not a proof of linearizability or exclusive regions.
use crate::{Error, ir::*, sexpr::Sexp};
use serde::{Deserialize, Serialize};

pub const HOST_CONTRACT: &str = "wlt-atomic-cells-v1";
pub const KERNEL_SOURCE: &str = include_str!("../proof/AtomicCells.lean");

#[derive(Clone, Debug, Serialize)]
pub struct AtomicCellDecl {
    pub name: String,
    pub memory: MemoryId,
    pub ty: NumType,
    pub offset: u32,
    #[serde(skip)]
    pub(crate) span: Span,
}
impl AtomicCellDecl {
    pub fn width(&self) -> u32 {
        if self.ty == NumType::I32 { 4 } else { 8 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub offset: u64,
    pub width: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub opcode: String,
    pub offset: u64,
    pub width: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certificate {
    pub version: u32,
    #[serde(rename = "memoryBytes")]
    pub memory_bytes: u64,
    pub cells: Vec<Cell>,
    pub accesses: Vec<Access>,
    #[serde(
        default,
        rename = "readonlyRegions",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub readonly_regions: Vec<crate::readonly::Region>,
    #[serde(
        default,
        rename = "readonlyAccesses",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub readonly_accesses: Vec<crate::readonly::Site>,
}

pub(crate) fn requested(source: &str) -> bool {
    fn walk(node: &Sexp) -> bool {
        if node.head().is_some_and(|h| h.starts_with('@')) {
            return false;
        }
        matches!(node.head(), Some("%race-free" | "race-free"))
            || node.list().is_some_and(|v| v.iter().any(walk))
    }
    crate::sexpr::parse(source).is_ok_and(|node| walk(&node))
}

pub(crate) fn cell_declaration(
    source: &str,
    node: &Sexp,
    memories: &[MemoryDecl],
) -> Result<AtomicCellDecl, Error> {
    let err = |message| Error::at(source, node.span, message).with_code("E_ATOMIC_CELL");
    let p = node
        .list()
        .ok_or_else(|| err("expected atomic cell declaration"))?;
    if p.len() != 5 {
        return Err(err(
            "expected (%atomic.cell $name $memory i32/i64 byte-offset)",
        ));
    }
    let name = p[1]
        .atom()
        .ok_or_else(|| err("expected cell name"))?
        .to_owned();
    let memory = memories
        .iter()
        .position(|m| Some(m.name.as_str()) == p[2].atom())
        .map(MemoryId)
        .ok_or_else(|| err("cell requires a named memory"))?;
    let ty = p[3]
        .atom()
        .and_then(NumType::parse)
        .filter(|t| matches!(t, NumType::I32 | NumType::I64))
        .ok_or_else(|| err("atomic cells support i32 or i64"))?;
    let offset = p[4]
        .atom()
        .and_then(crate::threads::index)
        .ok_or_else(|| err("cell offset must be an unsigned memory32 constant"))?;
    let cell = AtomicCellDecl {
        name,
        memory,
        ty,
        offset,
        span: node.span,
    };
    if !offset.is_multiple_of(cell.width())
        || u64::from(offset) + u64::from(cell.width())
            > u64::from(memories[memory.0].initial) * 65536
    {
        return Err(err(
            "atomic cell must be naturally aligned and contained in the initial memory",
        ));
    }
    Ok(cell)
}

pub(crate) fn compatible(op: &MemoryOp, cell: &AtomicCellDecl) -> bool {
    match op.name() {
        "memory.atomic.notify" => true, // Wait queues are not payload writes.
        "memory.atomic.wait32" => cell.ty == NumType::I32,
        "memory.atomic.wait64" => cell.ty == NumType::I64,
        name => {
            op.atomic()
                && op.bytes() == cell.width()
                && name.starts_with(&format!("{}.atomic.", cell.ty.wat()))
        }
    }
}

pub(crate) fn validate(source: &str, module: &Module) -> Result<(), Error> {
    certificate_inner(source, module).map(|_| ())
}

/// Rust structural validation and footprint extraction, not a Lean invocation.
/// Compilation and verified execution additionally run verify_certificate.
pub fn certificate(module: &Module) -> Result<Option<Certificate>, Error> {
    certificate_inner("", module)
}
fn certificate_inner(source: &str, module: &Module) -> Result<Option<Certificate>, Error> {
    if !module.race_free {
        return Ok(None);
    }
    let err = |span, message| Error::at(source, span, message).with_code("E_RACE_MODE");
    let extended = !module.readonly_regions.is_empty();
    let shared = module.memories.iter().position(|m| m.shared);
    if !extended && module.memories.len() > 1
        || extended && module.memories.iter().filter(|m| m.shared).count() != 1
    {
        return Err(err(
            module.race_span,
            "race-free mode permits one fixed shared source memory",
        ));
    }
    for m in &module.memories {
        if (!extended || m.shared)
            && (!m.shared || m.maximum != Some(m.initial) || !m.exports.is_empty())
            || extended && !m.shared && m.import.is_some()
        {
            return Err(err(
                module.race_span,
                "race-free memory must be shared, have equal initial/maximum limits, and have no raw exports",
            ));
        }
    }
    if module.atomic_cells.len() > 256 {
        return Err(err(
            module.race_span,
            "race-free mode supports at most 256 static cells",
        ));
    }
    let memory_bytes = shared.map_or(0, |i| u64::from(module.memories[i].initial) * 65536);
    let mut cells: Vec<Cell> = Vec::new();
    for cell in &module.atomic_cells {
        if Some(cell.memory.0) != shared
            || !matches!(cell.ty, NumType::I32 | NumType::I64)
            || !cell.offset.is_multiple_of(cell.width())
            || u64::from(cell.offset) + u64::from(cell.width()) > memory_bytes
        {
            return Err(err(
                cell.span,
                "cell is outside the certified memory layout",
            ));
        }
        let range = Cell {
            offset: u64::from(cell.offset),
            width: cell.width(),
        };
        if cells.iter().any(|c| {
            c.offset < range.offset + u64::from(range.width)
                && range.offset < c.offset + u64::from(c.width)
        }) {
            return Err(Error::at(
                source,
                cell.span,
                "atomic cell declarations overlap; reuse the same cell binding for aliases",
            )
            .with_code("E_RACE_LAYOUT"));
        }
        cells.push(range);
    }
    let mut readonly_regions = Vec::new();
    if module.readonly_regions.len() > 256 {
        return Err(err(
            module.race_span,
            "race-free mode supports at most 256 readonly regions",
        ));
    }
    for region in &module.readonly_regions {
        if Some(region.memory.0) != shared
            || region.length == 0
            || u64::from(region.offset) + u64::from(region.length) > memory_bytes
            || cells.iter().any(|c| {
                c.offset < u64::from(region.offset) + u64::from(region.length)
                    && u64::from(region.offset) < c.offset + u64::from(c.width)
            })
        {
            return Err(Error::at(
                source,
                region.span,
                "readonly region must lie inside shared memory and not overlap atomic cells",
            )
            .with_code("E_RACE_LAYOUT"));
        }
        readonly_regions.push(crate::readonly::Region {
            offset: u64::from(region.offset),
            length: u64::from(region.length),
        });
    }
    let mut readonly_accesses = Vec::new();
    let mut accesses = Vec::new();
    for function in &module.functions {
        if matches!(function.kind, FunctionKind::Import { .. }) {
            return Err(err(
                module.race_span,
                "race-free mode rejects host function imports, including unused imports",
            ));
        }
        if !function.exports.is_empty()
            && (!matches!(function.result, Type::Unit | Type::Num(_))
                || function
                    .params
                    .iter()
                    .any(|id| !matches!(function.locals[id.0].ty, Type::Num(_))))
        {
            return Err(err(
                module.race_span,
                "race-free ABI exports support numeric arguments and numeric/unit results; GC values remain agent-local",
            ));
        }
        let mut pending = function.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            if matches!(
                e.kind,
                ExprKind::TaskSpawn { .. } | ExprKind::TaskJoin { .. } | ExprKind::TaskScope { .. }
            ) {
                return Err(Error::at(
                    source,
                    e.span,
                    "raw memory instructions are outside the fixed atomic-cell contract",
                )
                .with_code("E_RACE_ACCESS"));
            }
            if let ExprKind::Wasm { instruction, args } = &e.kind {
                let private = extended
                    && instruction.memories.iter().all(|id| {
                        module
                            .memories
                            .get(id.0)
                            .is_some_and(|m| !m.shared && m.import.is_none())
                    });
                if let Some(load) = &instruction.readonly {
                    let region = module
                        .readonly_regions
                        .get(load.region)
                        .ok_or_else(|| err(e.span, "unknown readonly region"))?;
                    if !extended
                        || instruction.memories != [region.memory]
                        || !instruction.immediates.is_empty()
                        || crate::readonly::width(&instruction.opcode) != Some(load.width)
                        || load.width > region.length
                        || args.len() != 1
                        || args[0].ty != Type::Num(NumType::I32) && args[0].ty != Type::Never
                    {
                        return Err(err(e.span, "invalid bounded readonly load"));
                    }
                    readonly_accesses.push(crate::readonly::Site {
                        region: load.region,
                        opcode: instruction.opcode.clone(),
                        width: load.width,
                    });
                } else if !private {
                    return Err(Error::at(
                        source,
                        e.span,
                        "raw shared memory accesses require a checked readonly region",
                    )
                    .with_code("E_RACE_ACCESS"));
                }
            }
            if let ExprKind::Memory {
                op,
                memory,
                cell,
                offset,
                args,
            } = &e.kind
            {
                if extended && memory.is_some_and(|id| !module.memories[id.0].shared) {
                    pending.extend(e.children());
                    continue;
                }
                if op.fence() {
                    pending.extend(e.children());
                    continue;
                }
                let error = || {
                    Error::at(source, e.span, "race-free memory access requires a full-width (%atomic.at $cell), without pointer arithmetic or extra offsets").with_code("E_RACE_ACCESS")
                };
                let id = cell.ok_or_else(error)?;
                let cell = module.atomic_cells.get(id.0).ok_or_else(error)?;
                let address = args.first().ok_or_else(error)?;
                if *memory != Some(cell.memory)
                    || *offset != 0
                    || !compatible(op, cell)
                    || !matches!(&address.kind, ExprKind::Const { ty: NumType::I32, literal } if crate::threads::index(literal) == Some(cell.offset))
                {
                    return Err(error());
                }
                if op.name() != "memory.atomic.notify" {
                    accesses.push(Access {
                        opcode: op.name().into(),
                        offset: u64::from(cell.offset),
                        width: cell.width(),
                    });
                }
            }
            pending.extend(e.children());
        }
    }
    Ok(Some(Certificate {
        version: if extended { 2 } else { 1 },
        memory_bytes,
        cells,
        accesses,
        readonly_regions,
        readonly_accesses,
    }))
}

/// Independent kernel checking of a footprint certificate. This verifies the
/// abstract trace theorem's premises, not the extractor or host implementation.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub fn verify_certificate(certificate: &Certificate) -> Result<(), Error> {
    use std::io::Write;
    let mut data = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .map_err(|e| Error::message(e.to_string()))?;
    serde_json::to_writer(&mut data, certificate).map_err(|e| Error::message(e.to_string()))?;
    data.flush().map_err(|e| Error::message(e.to_string()))?;
    let mut kernel = tempfile::Builder::new()
        .suffix(".lean")
        .tempfile()
        .map_err(|e| Error::message(e.to_string()))?;
    write!(kernel, "{KERNEL_SOURCE}\ndef main := WltAtomicCells.run\n")
        .map_err(|e| Error::message(e.to_string()))?;
    kernel.flush().map_err(|e| Error::message(e.to_string()))?;
    let output = std::process::Command::new("lean")
        .arg(crate::verified::LEAN_VERSION)
        .arg("--run")
        .arg(kernel.path())
        .arg(data.path())
        .output()
        .map_err(|e| {
            Error::message(format!("could not start atomic-cell kernel: {e}"))
                .with_code("E_RACE_KERNEL")
        })?;
    if !output.status.success() {
        return Err(Error::message(format!(
            "atomic-cell kernel rejected certificate:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
        .with_code("E_RACE_KERNEL"));
    }
    Ok(())
}
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub fn verify_certificate(_: &Certificate) -> Result<(), Error> {
    Err(
        Error::message("atomic-cell proof verification requires the native Lean kernel".into())
            .with_code("E_HOST_UNAVAILABLE"),
    )
}
