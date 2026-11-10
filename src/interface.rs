//! Explicit portable views of nominal Wasm types; no identity or ownership erasure.
use crate::{Error, ir::*, sexpr::Sexp};

// wasmparser 0.262's Component validator permits 1..=32 flag names.
pub(crate) const MAX_FLAGS: usize = 32;

#[derive(Clone, Debug, serde::Serialize)]
pub enum AbiType {
    Bool,
    Char,
    S8,
    S16,
    Tuple,
    Variant(Vec<String>),
    U8,
    U16,
    U32,
    U64,
    Record(Vec<String>),
    Enum(Vec<String>),
    Flags(Vec<String>),
}

pub(crate) fn apply(source: &str, module: &mut Module, nodes: &[&Sexp]) -> Result<(), Error> {
    for node in nodes {
        let error = || {
            Error::at(source, node.span, "invalid @abi.type declaration").with_code("E_ABI_TYPE")
        };
        let p = node.list().ok_or_else(error)?;
        if p.len() != 3 {
            return Err(error());
        }
        let id = module
            .types
            .iter()
            .position(|t| Some(t.name.as_str()) == p[1].atom())
            .ok_or_else(error)?;
        if module.types[id].abi.is_some() {
            return Err(error());
        }
        let d = p[2].list().ok_or_else(error)?;
        let names = d
            .iter()
            .skip(1)
            .map(|n| n.text().map(str::to_owned).ok_or_else(error))
            .collect::<Result<Vec<_>, _>>()?;
        if names.iter().any(|n| !identifier(n))
            || names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != names.len()
        {
            return Err(error());
        }
        let view = match p[2].head() {
            Some(name @ ("bool" | "char" | "s8" | "s16")) if d.len()==1 && matches!(module.types[id].kind, TypeKind::Newtype(NumType::I32)) => match name {
                "bool" => AbiType::Bool, "char" => AbiType::Char, "s8" => AbiType::S8, _ => AbiType::S16,
            },
            Some("tuple") if d.len()==1 && matches!(&module.types[id].kind,TypeKind::Struct(f) if !f.is_empty() && f.iter().all(|f| !f.mutable)) => AbiType::Tuple,
            Some("variant") if !names.is_empty() && matches!(&module.types[id].kind,TypeKind::Variant(v) if v.len()==names.len() && v.iter().zip(&names).all(|(c,n)| c.name==*n && c.payloads.len()<=1)) => AbiType::Variant(names),
            Some("u8")
                if d.len() == 1
                    && matches!(module.types[id].kind, TypeKind::Newtype(NumType::I32)) =>
            {
                AbiType::U8
            }
            Some("u16")
                if d.len() == 1
                    && matches!(module.types[id].kind, TypeKind::Newtype(NumType::I32)) =>
            {
                AbiType::U16
            }
            Some("u32")
                if d.len() == 1
                    && matches!(module.types[id].kind, TypeKind::Newtype(NumType::I32)) =>
            {
                AbiType::U32
            }
            Some("u64")
                if d.len() == 1
                    && matches!(module.types[id].kind, TypeKind::Newtype(NumType::I64)) =>
            {
                AbiType::U64
            }
            Some("record")
                if !names.is_empty()
                    && matches!(&module.types[id].kind, TypeKind::Struct(fields) if fields.len()==names.len() && fields.iter().zip(&names).all(|(f,n)|!f.mutable && f.name.as_deref().map(|s|s.trim_start_matches('$'))==Some(n))) =>
            {
                AbiType::Record(names)
            }
            Some("enum")
                if !names.is_empty()
                    && matches!(&module.types[id].kind, TypeKind::Variant(v) if v.len()==names.len() && v.iter().zip(&names).all(|(v,n)|v.payloads.is_empty() && v.name==*n)) =>
            {
                AbiType::Enum(names)
            }
            Some("flags")
                if !names.is_empty()
                    && names.len() <= MAX_FLAGS
                    && matches!(module.types[id].kind,TypeKind::Newtype(NumType::I32)) =>
            {
                AbiType::Flags(names)
            }
            Some("flags")=>return Err(error().help("flags require 1 to 32 names and i32 storage, matching the pinned Component validator")),
            _ => return Err(error()),
        };
        module.types[id].abi = Some(view);
    }
    Ok(())
}
fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|s| {
            s.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
pub(crate) fn validate(source: &str, module: &mut Module) -> Result<(), Error> {
    crate::abi_limits::validate(source, module)
}

pub(crate) fn shape(m: &Module, id: TypeId) -> Option<String> {
    if m.types[id.0].abi.is_some()
        && let Some(f) = &m.types[id.0].abi_facts
    {
        return Some(f.shape.clone());
    }
    Some(match m.types[id.0].abi.as_ref()? {
        AbiType::Bool => "bool".into(),
        AbiType::Char => "char".into(),
        AbiType::S8 => "s8".into(),
        AbiType::S16 => "s16".into(),
        AbiType::Tuple => {
            let TypeKind::Struct(fields) = &m.types[id.0].kind else {
                unreachable!()
            };
            format!(
                "(tuple{})",
                fields
                    .iter()
                    .map(|f| format!(" {}", m.shape_name(&f.ty)))
                    .collect::<String>()
            )
        }
        AbiType::Variant(names) => {
            let TypeKind::Variant(cases) = &m.types[id.0].kind else {
                unreachable!()
            };
            format!(
                "(variant {})",
                cases
                    .iter()
                    .zip(names)
                    .map(|(c, n)| format!(
                        "({n}{})",
                        c.payloads
                            .iter()
                            .map(|t| format!(" {}", m.shape_name(t)))
                            .collect::<String>()
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }
        AbiType::U8 => "u8".into(),
        AbiType::U16 => "u16".into(),
        AbiType::U32 => "u32".into(),
        AbiType::U64 => "u64".into(),
        AbiType::Flags(n) => format!("(flags {})", n.join(" ")),
        AbiType::Enum(n) => format!("(enum {})", n.join(" ")),
        AbiType::Record(names) => {
            let TypeKind::Struct(fields) = &m.types[id.0].kind else {
                unreachable!()
            };
            format!(
                "(record {})",
                fields
                    .iter()
                    .zip(names)
                    .map(|(f, n)| format!("({n} {})", m.shape_name(&f.ty)))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }
    })
}
