//! Linear-memory capabilities and the original Wasm threads instruction set.
//! Memories are module capabilities, never owned GC values or transferable refs.
use crate::{Error, ir::NumType, sexpr::Sexp};
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryId(pub usize);

#[derive(Clone, Debug, Serialize)]
pub struct MemoryImport {
    pub module: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryDecl {
    pub name: String,
    pub initial: u32,
    pub maximum: Option<u32>,
    pub shared: bool,
    pub import: Option<MemoryImport>,
    pub exports: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryOp {
    name: String,
    inputs: Vec<NumType>,
    output: Option<NumType>,
    bytes: u32,
    pub(crate) reads: bool,
    pub(crate) writes: bool,
    pub(crate) blocks: bool,
}
impl MemoryOp {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn inputs(&self) -> &[NumType] {
        &self.inputs
    }
    pub fn output(&self) -> Option<NumType> {
        self.output
    }
    pub fn bytes(&self) -> u32 {
        self.bytes
    }
    pub fn atomic(&self) -> bool {
        self.name.contains("atomic")
    }
    pub fn fence(&self) -> bool {
        self.name == "atomic.fence"
    }
    pub fn synchronizes(&self, shared: bool) -> bool {
        self.atomic() || shared
    }
}

pub(crate) fn lookup(name: &str) -> Option<&'static MemoryOp> {
    if !name.contains("atomic") && !matches!(name, "memory.size" | "memory.grow") {
        return None;
    }
    static OPS: OnceLock<Vec<MemoryOp>> = OnceLock::new();
    OPS.get_or_init(|| {
        use NumType::{I32, I64};
        let mut ops = vec![];
        let mut add = |name: String, inputs: Vec<NumType>, output, bytes, reads, writes, blocks| {
            ops.push(MemoryOp {
                name,
                inputs,
                output,
                bytes,
                reads,
                writes,
                blocks,
            });
        };
        add("atomic.fence".into(), vec![], None, 0, false, false, false);
        add(
            "memory.atomic.notify".into(),
            vec![I32, I32],
            Some(I32),
            4,
            false,
            false,
            false,
        );
        add(
            "memory.atomic.wait32".into(),
            vec![I32, I32, I64],
            Some(I32),
            4,
            true,
            false,
            true,
        );
        add(
            "memory.atomic.wait64".into(),
            vec![I32, I64, I64],
            Some(I32),
            8,
            true,
            false,
            true,
        );
        for (ty, widths) in [(I32, &[1, 2, 4][..]), (I64, &[1, 2, 4, 8][..])] {
            for &bytes in widths {
                let full = bytes == if ty == I32 { 4 } else { 8 };
                let load = if full {
                    "load".into()
                } else {
                    format!("load{}_u", bytes * 8)
                };
                let store = if full {
                    "store".into()
                } else {
                    format!("store{}", bytes * 8)
                };
                add(
                    format!("{}.atomic.{load}", ty.wat()),
                    vec![I32],
                    Some(ty),
                    bytes,
                    true,
                    false,
                    false,
                );
                add(
                    format!("{}.atomic.{store}", ty.wat()),
                    vec![I32, ty],
                    None,
                    bytes,
                    false,
                    true,
                    false,
                );
                for operation in ["add", "sub", "and", "or", "xor", "xchg", "cmpxchg"] {
                    let suffix = if full {
                        format!("rmw.{operation}")
                    } else {
                        format!("rmw{}.{operation}_u", bytes * 8)
                    };
                    let mut inputs = vec![I32, ty];
                    if operation == "cmpxchg" {
                        inputs.push(ty);
                    }
                    add(
                        format!("{}.atomic.{suffix}", ty.wat()),
                        inputs,
                        Some(ty),
                        bytes,
                        true,
                        true,
                        false,
                    );
                }
            }
        }
        add(
            "memory.size".into(),
            vec![],
            Some(I32),
            0,
            true,
            false,
            false,
        );
        add(
            "memory.grow".into(),
            vec![I32],
            Some(I32),
            0,
            true,
            true,
            false,
        );
        ops
    })
    .iter()
    .find(|op| op.name == name)
}

pub(crate) fn immediate(node: &Sexp) -> bool {
    node.atom()
        .is_some_and(|a| a.starts_with("offset=") || a.starts_with("align="))
}

/// WAT selectors precede operands. Arity disambiguates Walz's bare scalar/local
/// sugar: (i32.atomic.load 0) has one address, (... $mem 0) has a selector.
pub(crate) fn selector(items: &[Sexp]) -> Option<usize> {
    let op = lookup(items.first()?.atom()?)?;
    if op.fence() {
        return None;
    }
    let count = items[1..].iter().filter(|n| !immediate(n)).count();
    (count > op.inputs.len() && items.get(1)?.atom().is_some() && !immediate(&items[1]))
        .then_some(1)
}

pub(crate) fn index(text: &str) -> Option<u32> {
    let buffer = wast::parser::ParseBuffer::new(text).ok()?;
    wast::parser::parse::<u32>(&buffer).ok()
}

pub(crate) fn declaration(
    source: &str,
    node: &Sexp,
    import: Option<MemoryImport>,
) -> Result<MemoryDecl, Error> {
    let error = |message: &str| Error::at(source, node.span, message).with_code("E_MEMORY_DECL");
    let parts = node
        .list()
        .ok_or_else(|| error("expected memory declaration"))?;
    let Some(name) = parts
        .get(1)
        .and_then(Sexp::atom)
        .filter(|s| s.starts_with('$'))
    else {
        // Anonymous native WAT memories retain the existing opaque macro path.
        return Err(Error::at(
            source,
            node.span,
            "unsupported declaration in the typed core: memory requires a $name",
        ));
    };
    if name.starts_with("$__") {
        return Err(error("the $__ prefix is reserved"));
    }
    let name = name.to_owned();
    let mut exports = vec![];
    let mut limits = vec![];
    let mut shared = false;
    for part in &parts[2..] {
        if part.head() == Some("export") {
            if !limits.is_empty() || shared {
                return Err(error("memory exports must precede limits"));
            }
            let fields = part.list().unwrap();
            if fields.len() != 2 {
                return Err(error("expected (export \"name\")"));
            }
            exports.push(
                fields[1]
                    .text()
                    .ok_or_else(|| error("expected memory export string"))?
                    .to_owned(),
            );
        } else if part.atom() == Some("shared") && !shared {
            shared = true;
        } else if let Some(n) = part.atom().and_then(index) {
            if shared {
                return Err(error("memory limits must precede shared"));
            }
            limits.push(n);
        } else {
            return Err(error(
                "typed memories support named memory32 declarations with page limits and optional shared",
            ));
        }
    }
    if limits.is_empty()
        || limits.len() > 2
        || limits.iter().any(|n| *n > 65536)
        || limits.get(1).is_some_and(|max| *max < limits[0])
        || shared && limits.len() != 2
    {
        return Err(error(
            "memory32 requires initial <= maximum <= 65536 pages; shared memory requires a maximum",
        ));
    }
    Ok(MemoryDecl {
        name,
        initial: limits[0],
        maximum: limits.get(1).copied(),
        shared,
        import,
        exports,
    })
}
