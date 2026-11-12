//! Bounded, memoized facts for portable Canonical ABI type graphs.
use crate::{Error, interface::AbiType, ir::*};

const MAX_FLAT: usize = 1024;
const MAX_INLINE: u32 = 16 * 1024 * 1024;
const MAX_SHAPE: usize = 64 * 1024;
const MAX_TOTAL_SHAPE: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Facts {
    pub size: u32,
    pub align: u32,
    pub flat: usize,
    pub resource: bool,
    pub shape: String,
    shared: bool,
    depth: usize,
}
impl Facts {
    fn scalar(size: u32, align: u32, flat: usize, shape: impl Into<String>) -> Self {
        Self {
            size,
            align,
            flat,
            resource: false,
            shape: shape.into(),
            shared: true,
            depth: 0,
        }
    }
}

pub(crate) fn facts<'a>(m: &'a Module, t: &Type) -> Option<&'a Facts> {
    match t {
        Type::Resource(id)
        | Type::Value(id)
        | Type::Newtype(id)
        | Type::Ref {
            ty: id,
            nullable: false,
        } => m.types[id.0].abi_facts.as_ref(),
        _ => None,
    }
}

struct Graph<'a> {
    source: &'a str,
    module: &'a Module,
    states: Vec<u8>,
    memo: Vec<Option<Facts>>,
    total: usize,
    #[cfg(test)]
    visits: usize,
}
impl Graph<'_> {
    fn limit(&self) -> Error {
        Error::at(self.source, Span::default(), "ABI type exceeds implementation limits (1024 flat slots, 16 MiB inline size, 64 KiB expanded shape, depth 64)").with_code("E_ABI_LIMIT")
    }
    fn aligned(&self, n: u32, a: u32) -> Result<u32, Error> {
        n.checked_add(a - 1)
            .map(|n| n & !(a - 1))
            .ok_or_else(|| self.limit())
    }
    fn ty(&mut self, ty: &Type, level: usize) -> Result<Option<Facts>, Error> {
        Ok(match ty {
            Type::Unit => Some(Facts::scalar(1, 1, 1, "unit")),
            Type::Num(NumType::V128) => None,
            Type::Num(n) => Some(Facts::scalar(
                if matches!(n, NumType::I64 | NumType::F64) {
                    8
                } else {
                    4
                },
                if matches!(n, NumType::I64 | NumType::F64) {
                    8
                } else {
                    4
                },
                1,
                n.wat(),
            )),
            Type::Resource(id)
            | Type::Newtype(id)
            | Type::Value(id)
            | Type::Ref {
                ty: id,
                nullable: false,
            } => self.node(*id, level)?,
            _ => None,
        })
    }
    fn node(&mut self, id: TypeId, level: usize) -> Result<Option<Facts>, Error> {
        if self.states[id.0] == 2 {
            return Ok(self.memo[id.0].clone());
        }
        if self.states[id.0] == 1 {
            return Ok(None);
        }
        if level >= 64 {
            return Err(self.limit());
        }
        #[cfg(test)]
        {
            self.visits += 1;
        }
        self.states[id.0] = 1;
        let decl = self.module.types[id.0].clone();
        let mut children = Vec::new();
        let mut shape = String::new();
        let mut sequence = false;
        let mut variant_tag = None;
        let mut fixed = None;
        let mut shared_only = false;
        match (&decl.abi, &decl.kind) {
            (Some(AbiType::Record(names) | AbiType::Variant(names)), kind) => {
                shared_only = true;
                let is_record = matches!(decl.abi, Some(AbiType::Record(_)));
                sequence = is_record;
                if !is_record {
                    variant_tag = Some(if names.len() <= 256 {
                        1
                    } else if names.len() <= 65536 {
                        2
                    } else {
                        4
                    });
                }
                shape.push_str(if is_record { "(record" } else { "(variant" });
                let fields: Vec<Vec<Type>> = match kind {
                    TypeKind::Struct(fields) => fields.iter().map(|f| vec![f.ty.clone()]).collect(),
                    TypeKind::Variant(cases) => cases.iter().map(|c| c.payloads.clone()).collect(),
                    _ => unreachable!(),
                };
                for (name, fields) in names.iter().zip(fields) {
                    shape.push_str(&format!(" ({name}"));
                    for t in fields {
                        let Some(f) = self.ty(&t, level + 1)? else {
                            self.states[id.0] = 2;
                            return Ok(None);
                        };
                        if !f.shared {
                            self.states[id.0] = 2;
                            return Ok(None);
                        }
                        if shape.len() + f.shape.len() + 2 > MAX_SHAPE {
                            return Err(self.limit());
                        }
                        shape.push(' ');
                        shape.push_str(&f.shape);
                        children.push(f);
                    }
                    shape.push(')');
                }
                shape.push(')');
            }
            (Some(AbiType::Tuple), TypeKind::Struct(fields)) => {
                sequence = true;
                shared_only = true;
                shape.push_str("(tuple");
                for field in fields {
                    let Some(f) = self.ty(&field.ty, level + 1)? else {
                        self.states[id.0] = 2;
                        return Ok(None);
                    };
                    if !f.shared {
                        self.states[id.0] = 2;
                        return Ok(None);
                    }
                    if shape.len() + f.shape.len() + 2 > MAX_SHAPE {
                        return Err(self.limit());
                    }
                    shape.push(' ');
                    shape.push_str(&f.shape);
                    children.push(f);
                }
                shape.push(')');
            }
            (Some(abi), _) => {
                let (n, s) = match abi {
                    AbiType::Bool => (1, "bool".into()),
                    AbiType::Char => (4, "char".into()),
                    AbiType::S8 => (1, "s8".into()),
                    AbiType::S16 => (2, "s16".into()),
                    AbiType::U8 => (1, "u8".into()),
                    AbiType::U16 => (2, "u16".into()),
                    AbiType::U32 => (4, "u32".into()),
                    AbiType::U64 => (8, "u64".into()),
                    AbiType::Enum(ns) => (
                        if ns.len() <= 256 {
                            1
                        } else if ns.len() <= 65536 {
                            2
                        } else {
                            4
                        },
                        format!("(enum {})", ns.join(" ")),
                    ),
                    AbiType::Flags(ns) => (
                        if ns.len() <= 8 {
                            1
                        } else if ns.len() <= 16 {
                            2
                        } else {
                            4
                        },
                        format!("(flags {})", ns.join(" ")),
                    ),
                    _ => unreachable!(),
                };
                fixed = Some(Facts::scalar(n, n, 1, s));
            }
            (_, TypeKind::Resource { kind, .. }) => {
                let mut f = Facts::scalar(4, 4, 1, format!("(resource {kind})"));
                f.shared = false;
                f.resource = true;
                fixed = Some(f);
            }
            (_, TypeKind::Newtype(n)) => {
                fixed = self.ty(&Type::Num(*n), level + 1)?;
            }
            (_, TypeKind::Value(ValueType::Bytes | ValueType::Text)) => {
                fixed = Some(Facts::scalar(
                    8,
                    4,
                    2,
                    if matches!(decl.kind, TypeKind::Value(ValueType::Bytes)) {
                        "bytes"
                    } else {
                        "text"
                    },
                ));
            }
            (_, TypeKind::Value(v)) => {
                let (name, ts) = match v {
                    ValueType::List(t) => ("list", vec![t]),
                    ValueType::Option(t) => ("option", vec![t]),
                    ValueType::Future(t) => ("future", vec![t]),
                    ValueType::Stream(t) => ("stream", vec![t]),
                    ValueType::Pair { first, second } => ("pair", vec![first, second]),
                    ValueType::Result { ok, err } => ("result", vec![ok, err]),
                    _ => {
                        self.states[id.0] = 2;
                        return Ok(None);
                    }
                };
                shape.push('(');
                shape.push_str(name);
                for t in ts {
                    let Some(f) = self.ty(t, level + 1)? else {
                        self.states[id.0] = 2;
                        return Ok(None);
                    };
                    if shape.len() + f.shape.len() + 2 > MAX_SHAPE {
                        return Err(self.limit());
                    }
                    shape.push(' ');
                    shape.push_str(&f.shape);
                    if !matches!(v, ValueType::Result { .. }) || *t != Type::Unit {
                        children.push(f);
                    }
                }
                shape.push(')');
                match v {
                    ValueType::List(_) | ValueType::Future(_) | ValueType::Stream(_) => {
                        if !children[0].shared {
                            self.states[id.0] = 2;
                            return Ok(None);
                        }
                        let endpoint = matches!(v, ValueType::Future(_) | ValueType::Stream(_));
                        let mut f = Facts::scalar(
                            if endpoint { 4 } else { 8 },
                            4,
                            if endpoint { 1 } else { 2 },
                            shape.clone(),
                        );
                        f.shared = !endpoint;
                        f.depth = children[0].depth + 1;
                        fixed = Some(f);
                    }
                    ValueType::Pair { .. } => sequence = true,
                    _ => variant_tag = Some(1),
                }
            }
            _ => {
                self.states[id.0] = 2;
                return Ok(None);
            }
        }
        let f = if let Some(f) = fixed {
            f
        } else {
            let alignment = children
                .iter()
                .map(|f| f.align)
                .max()
                .unwrap_or(1)
                .max(variant_tag.unwrap_or(1));
            let mut size = 0u32;
            let mut flat = 0usize;
            if sequence {
                for child in &children {
                    size = self
                        .aligned(size, child.align)?
                        .checked_add(child.size)
                        .ok_or_else(|| self.limit())?;
                    flat = flat.checked_add(child.flat).ok_or_else(|| self.limit())?;
                }
            } else {
                let tag = variant_tag.unwrap();
                size = self
                    .aligned(tag, alignment)?
                    .checked_add(children.iter().map(|f| f.size).max().unwrap_or(0))
                    .ok_or_else(|| self.limit())?;
                flat = 1 + children.iter().map(|f| f.flat).max().unwrap_or(0);
            }
            Facts {
                size: self.aligned(size, alignment)?,
                align: alignment,
                flat,
                resource: children.iter().any(|f| f.resource),
                shape,
                shared: shared_only || children.iter().all(|f| f.shared),
                depth: 1 + children.iter().map(|f| f.depth).max().unwrap_or(0),
            }
        };
        if f.size > MAX_INLINE || f.flat > MAX_FLAT || f.shape.len() > MAX_SHAPE || f.depth >= 64 {
            return Err(self.limit());
        }
        self.total = self
            .total
            .checked_add(f.shape.len())
            .ok_or_else(|| self.limit())?;
        if self.total > MAX_TOTAL_SHAPE {
            return Err(self.limit());
        }
        self.memo[id.0] = Some(f.clone());
        self.states[id.0] = 2;
        Ok(Some(f))
    }
}

pub(crate) fn validate(source: &str, module: &mut Module) -> Result<(), Error> {
    let mut graph = Graph {
        source,
        module,
        states: vec![0; module.types.len()],
        memo: vec![None; module.types.len()],
        total: 0,
        #[cfg(test)]
        visits: 0,
    };
    for id in 0..module.types.len() {
        let f = graph.node(TypeId(id), 0)?;
        if module.types[id].abi.is_some() && f.is_none() {
            return Err(Error::at(
                source,
                Span::default(),
                format!(
                    "non-portable or recursive ABI type {}",
                    module.types[id].name
                ),
            )
            .with_code("E_ABI_TYPE"));
        }
    }
    let memo = graph.memo;
    for (decl, f) in module.types.iter_mut().zip(memo) {
        decl.abi_facts = f;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_type_graph_is_visited_once_per_vertex() {
        let mut text =
            "(module (type $T0 (struct (field $value i32))) (@abi.type $T0 (record \"value\"))"
                .to_owned();
        for n in 1..9 {
            text += &format!(
                "(type $T{n} (struct (field (ref $T{})) (field (ref $T{})))) (@abi.type $T{n} (tuple))",
                n - 1,
                n - 1
            );
        }
        text.push(')');
        let module = crate::check(&text).unwrap();
        let mut graph = Graph {
            source: &text,
            module: &module,
            states: vec![0; module.types.len()],
            memo: vec![None; module.types.len()],
            total: 0,
            visits: 0,
        };
        for id in 0..module.types.len() {
            graph.node(TypeId(id), 0).unwrap();
        }
        assert_eq!(graph.visits, module.types.len());
    }
}
