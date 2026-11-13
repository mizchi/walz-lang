//! GC translation-validation contract. GC recursion groups and their indices
//! remain byte-for-byte intact; only trailing function-type interning, local
//! grouping and debug names are normalized. No GC type is merged or reordered.
use std::error::Error;
use wasm_encoder::{
    CodeSection, Encode, ExportKind, ExportSection, Function, FunctionSection, Module, RawSection,
    ValType,
};
use wasmparser::{CompositeInnerType, Parser, Payload};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn heap(h: wasmparser::HeapType, count: u32) -> Result<wasm_encoder::HeapType> {
    use wasm_encoder::{AbstractHeapType as A, HeapType as H};
    use wasmparser::{AbstractHeapType as P, HeapType as Q, UnpackedIndex};
    Ok(match h {
        Q::Concrete(UnpackedIndex::Module(i)) if i < count => H::Concrete(i),
        Q::Abstract { shared: false, ty } => H::Abstract {
            shared: false,
            ty: match ty {
                P::Any => A::Any,
                P::Eq => A::Eq,
                P::I31 => A::I31,
                P::Struct => A::Struct,
                P::Array => A::Array,
                P::None => A::None,
                _ => return Err("GC parity: unsupported heap type".into()),
            },
        },
        _ => return Err("GC parity: unsupported heap reference".into()),
    })
}
fn val(t: wasmparser::ValType, count: u32) -> Result<ValType> {
    Ok(match t {
        wasmparser::ValType::I32 => ValType::I32,
        wasmparser::ValType::I64 => ValType::I64,
        wasmparser::ValType::Ref(r) => ValType::Ref(wasm_encoder::RefType {
            nullable: r.is_nullable(),
            heap_type: heap(r.heap_type(), count)?,
        }),
        _ => return Err("GC parity: unsupported value type".into()),
    })
}
fn storage(t: wasmparser::StorageType, count: u32) -> Result<()> {
    if let wasmparser::StorageType::Val(t) = t {
        val(t, count)?;
    }
    Ok(())
}
pub fn canonical(bytes: &[u8]) -> Result<Vec<u8>> {
    wasmparser::Validator::new().validate_all(bytes)?;
    let mut gc_groups = Vec::new();
    let mut gc_count = 0u32;
    let mut signatures = Vec::new();
    let mut indices = Vec::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::Version {
                encoding: wasmparser::Encoding::Module,
                ..
            }
            | Payload::End(_) => (),
            Payload::CustomSection(c) if c.name() == "name" => (),
            Payload::TypeSection(reader) => {
                let end = reader.range().end as usize;
                let groups = reader
                    .into_iter_with_offsets()
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (i, (_, g)) in groups.iter().enumerate() {
                    if g.types().all(|t| {
                        matches!(
                            t.composite_type.inner,
                            CompositeInnerType::Struct(_) | CompositeInnerType::Array(_)
                        )
                    }) {
                        if !signatures.is_empty() {
                            return Err("GC parity: GC type after function pool".into());
                        }
                        gc_count += g.types().len() as u32;
                        let start = groups[i].0 as usize;
                        let finish = groups.get(i + 1).map_or(end, |p| p.0 as usize);
                        gc_groups.push(bytes[start..finish].to_vec());
                    } else {
                        let mut it = g.types();
                        let t = it.next().ok_or("GC parity: empty group")?;
                        if it.next().is_some()
                            || g.is_explicit_rec_group()
                            || !t.is_final
                            || !t.supertype_idxs.is_empty()
                        {
                            return Err("GC parity: unsupported function group".into());
                        }
                        let CompositeInnerType::Func(f) = &t.composite_type.inner else {
                            return Err("GC parity: non-function type".into());
                        };
                        if f.results().len() > 1 {
                            return Err("GC parity: multi-result".into());
                        }
                        signatures.push((
                            f.params()
                                .iter()
                                .copied()
                                .map(|t| val(t, gc_count))
                                .collect::<Result<Vec<_>>>()?,
                            f.results()
                                .iter()
                                .copied()
                                .map(|t| val(t, gc_count))
                                .collect::<Result<Vec<_>>>()?,
                        ));
                    }
                }
                for (_, g) in &groups {
                    for t in g.types() {
                        if t.composite_type.shared
                            || t.composite_type.descriptor_idx.is_some()
                            || t.composite_type.describes_idx.is_some()
                        {
                            return Err("GC parity: shared/described type".into());
                        }
                        match &t.composite_type.inner {
                            CompositeInnerType::Struct(s) => {
                                for f in &s.fields {
                                    storage(f.element_type, gc_count)?;
                                }
                            }
                            CompositeInnerType::Array(a) => storage(a.0.element_type, gc_count)?,
                            _ => (),
                        }
                    }
                }
            }
            Payload::FunctionSection(r) => {
                indices = r.into_iter().collect::<std::result::Result<Vec<_>, _>>()?
            }
            Payload::ExportSection(r) => {
                for e in r {
                    let e = e?;
                    if e.kind != wasmparser::ExternalKind::Func {
                        return Err("GC parity: non-function export".into());
                    }
                    exports.export(e.name, ExportKind::Func, e.index);
                }
            }
            Payload::CodeSectionStart { .. } => (),
            Payload::CodeSectionEntry(body) => {
                let mut locals = Vec::new();
                for l in body.get_locals_reader()? {
                    let (n, t) = l?;
                    if n > 1024 - locals.len() as u32 {
                        return Err("GC parity: local limit".into());
                    }
                    locals.extend(std::iter::repeat_n((1, val(t, gc_count)?), n as usize));
                }
                let mut r = body.get_operators_reader()?;
                while !r.eof() {
                    let at = r.original_position() as usize;
                    let op = r.read()?;
                    match &op {
                        wasmparser::Operator::Block { blockty }
                        | wasmparser::Operator::Loop { blockty }
                        | wasmparser::Operator::If { blockty } => match blockty {
                            wasmparser::BlockType::Empty => (),
                            wasmparser::BlockType::Type(t) => {
                                val(*t, gc_count)?;
                            }
                            _ => return Err("GC parity: indexed block type".into()),
                        },
                        wasmparser::Operator::TypedSelect { ty } => {
                            val(*ty, gc_count)?;
                        }
                        wasmparser::Operator::RefNull { hty }
                        | wasmparser::Operator::RefTestNonNull { hty }
                        | wasmparser::Operator::RefTestNullable { hty }
                        | wasmparser::Operator::RefCastNonNull { hty }
                        | wasmparser::Operator::RefCastNullable { hty } => {
                            heap(*hty, gc_count)?;
                        }
                        _ => (),
                    }
                    let allowed = match bytes[at] {
                        0
                        | 1
                        | 2
                        | 3
                        | 4
                        | 5
                        | 11
                        | 12
                        | 13
                        | 15
                        | 16
                        | 26
                        | 27
                        | 28
                        | 32..=34
                        | 65
                        | 66
                        | 69..=90
                        | 106..=118
                        | 124..=136
                        | 167
                        | 172
                        | 173
                        | 208
                        | 209
                        | 211
                        | 212 => true,
                        251 => matches!(bytes[at+1],0..=8|11..=17|20..=23|28..=30),
                        _ => false,
                    };
                    if !allowed {
                        return Err("GC parity: unsupported instruction".into());
                    }
                }
                let mut raw = body.get_binary_reader_for_operators()?;
                let ops = raw.read_bytes(raw.bytes_remaining())?;
                let mut f = Function::new(locals);
                f.raw(ops.iter().copied());
                code.function(&f);
            }
            _ => return Err("GC parity: unsupported section".into()),
        }
    }
    let mut types = Vec::new();
    ((gc_groups.len() + indices.len()) as u32).encode(&mut types);
    for group in gc_groups {
        types.extend(group);
    }
    let mut functions = FunctionSection::new();
    for (i, index) in indices.iter().copied().enumerate() {
        let f = signatures
            .get(
                index
                    .checked_sub(gc_count)
                    .ok_or("GC parity: non-function signature")? as usize,
            )
            .ok_or("GC parity: function signature index")?;
        types.push(96);
        f.0.encode(&mut types);
        f.1.encode(&mut types);
        functions.function(gc_count + i as u32);
    }
    let mut module = Module::new();
    module
        .section(&RawSection {
            id: 1,
            data: &types,
        })
        .section(&functions)
        .section(&exports)
        .section(&code);
    let result = module.finish();
    wasmparser::Validator::new().validate_all(&result)?;
    Ok(result)
}
