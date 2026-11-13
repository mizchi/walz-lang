//! Independent test oracle for the bounded GC IR schema.
use serde::Deserialize;
use wasm_encoder::*;

#[derive(Deserialize)]
pub struct Field {
    pub storage: i32,
    pub mutable: i32,
}
#[derive(Deserialize)]
pub struct Ty {
    pub kind: i32,
    pub fields: Vec<Field>,
    pub parent: i32,
    pub finality: i32,
    pub start: usize,
    pub end: usize,
    pub explicit: i32,
}
#[derive(Deserialize)]
pub struct Op {
    pub op: u32,
    pub immediate: String,
    #[serde(default)]
    pub start: i32,
    #[serde(default)]
    pub end: i32,
}
#[derive(Deserialize)]
pub struct Raw {
    pub types: Vec<Ty>,
    pub params: Vec<i32>,
    pub locals: Vec<i32>,
    pub result: i32,
    pub code: Vec<Op>,
}

fn heap(h: i32) -> Option<HeapType> {
    Some(if (0..1024).contains(&h) {
        HeapType::Concrete(h as u32)
    } else {
        HeapType::Abstract {
            shared: false,
            ty: match h {
                -18 => AbstractHeapType::Any,
                -19 => AbstractHeapType::Eq,
                -20 => AbstractHeapType::I31,
                -21 => AbstractHeapType::Struct,
                -22 => AbstractHeapType::Array,
                -23 => AbstractHeapType::None,
                _ => return None,
            },
        }
    })
}
fn val(t: i32) -> Option<ValType> {
    match t {
        127 => Some(ValType::I32),
        126 => Some(ValType::I64),
        466..=2559 => Some(ValType::Ref(RefType {
            nullable: t & 1 != 0,
            heap_type: heap((t - 512) >> 1)?,
        })),
        _ => None,
    }
}
fn field(f: &Field) -> Option<FieldType> {
    if !matches!(f.mutable, 0 | 1) {
        return None;
    }
    Some(FieldType {
        element_type: match f.storage {
            120 => StorageType::I8,
            119 => StorageType::I16,
            t => StorageType::Val(val(t)?),
        },
        mutable: f.mutable == 1,
    })
}
fn subtype(t: &Ty) -> Option<SubType> {
    if !matches!(t.finality, 0 | 1) || t.parent < -1 {
        return None;
    }
    let fields = t.fields.iter().map(field).collect::<Option<Vec<_>>>()?;
    let inner = match t.kind {
        0 => CompositeInnerType::Struct(StructType {
            fields: fields.into_boxed_slice(),
        }),
        1 if fields.len() == 1 => CompositeInnerType::Array(ArrayType(fields[0])),
        _ => return None,
    };
    Some(SubType {
        is_final: t.finality == 1,
        supertype_idxs: if t.parent < 0 {
            vec![]
        } else {
            vec![t.parent as u32]
        },
        composite_type: CompositeType {
            inner,
            shared: false,
            descriptor: None,
            describes: None,
        },
    })
}
fn code(f: &mut Function, op: &Op) -> Option<()> {
    if op.start < 0 || op.end < op.start || op.end > 1048576 {
        return None;
    }
    let imm = op.immediate.parse::<i64>().ok()?;
    let first = imm as u32;
    let second = ((imm as u64) >> 32) as u32;
    let mut bytes = Vec::new();
    if op.op >= 256 {
        bytes.push(251);
        (op.op - 256).encode(&mut bytes);
        match op.op - 256 {
            2..=5 | 8 | 17 => {
                first.encode(&mut bytes);
                second.encode(&mut bytes);
            }
            0 | 1 | 6 | 7 | 11..=14 | 16 => {
                if second != 0 {
                    return None;
                }
                first.encode(&mut bytes);
            }
            20..=23 => {
                heap(i32::try_from(imm).ok()?)?;
                imm.encode(&mut bytes);
            }
            15 | 28..=30 => {
                if imm != 0 {
                    return None;
                }
            }
            _ => return None,
        }
    } else {
        bytes.push(op.op.try_into().ok()?);
        match op.op {
            2..=4 => {
                if imm == 64 {
                    bytes.push(64);
                } else {
                    val(i32::try_from(imm).ok()?)?.encode(&mut bytes);
                }
            }
            28 => {
                bytes.push(1);
                val(i32::try_from(imm).ok()?)?.encode(&mut bytes);
            }
            208 => {
                heap(i32::try_from(imm).ok()?)?;
                imm.encode(&mut bytes);
            }
            12 | 13 | 16 | 32..=34 => u32::try_from(imm).ok()?.encode(&mut bytes),
            65 => i32::try_from(imm).ok()?.encode(&mut bytes),
            66 => imm.encode(&mut bytes),
            0
            | 1
            | 5
            | 11
            | 15
            | 26
            | 27
            | 69..=90
            | 106..=118
            | 124..=136
            | 167
            | 172
            | 173
            | 209
            | 211
            | 212 => {
                if imm != 0 {
                    return None;
                }
            }
            _ => return None,
        }
    }
    f.raw(bytes);
    Some(())
}
fn encode(raw: &Raw) -> Option<Vec<u8>> {
    if raw.types.len() > 1024
        || raw.params.len() > 256
        || raw.locals.len() > 1024
        || raw.code.len() > 262144
    {
        return None;
    }
    let mut types = TypeSection::new();
    let mut index = 0;
    let mut fields = 0;
    while index < raw.types.len() {
        let d = &raw.types[index];
        if d.start != index
            || d.end <= index
            || d.end > raw.types.len()
            || !matches!(d.explicit, 0 | 1)
            || (d.explicit == 0 && d.end != index + 1)
        {
            return None;
        }
        let mut group = Vec::new();
        for (i, t) in raw.types[index..d.end].iter().enumerate() {
            fields += t.fields.len();
            if fields > 8192
                || t.fields.len() > 256
                || t.start != index
                || t.end != d.end
                || t.explicit != d.explicit
                || t.parent >= (index + i) as i32
            {
                return None;
            }
            // The supported type environment permits field references only to
            // an earlier type or to members of the same recursion group.
            if t.fields
                .iter()
                .any(|f| f.storage >= 512 && ((f.storage - 512) >> 1) as usize >= d.end)
            {
                return None;
            }
            group.push(subtype(t)?);
        }
        if d.explicit != 0 {
            types.ty().rec(group);
        } else {
            types.ty().subtype(&group[0]);
        }
        index = d.end;
    }
    let params = raw
        .params
        .iter()
        .copied()
        .map(val)
        .collect::<Option<Vec<_>>>()?;
    let locals = raw
        .locals
        .iter()
        .copied()
        .map(|t| {
            let v = val(t)?;
            if matches!(v,ValType::Ref(r) if !r.nullable) {
                return None;
            }
            Some((1, v))
        })
        .collect::<Option<Vec<_>>>()?;
    let results = if raw.result == 64 {
        vec![]
    } else {
        vec![val(raw.result)?]
    };
    types.ty().function(params, results);
    let mut functions = FunctionSection::new();
    functions.function(raw.types.len() as u32);
    let mut f = Function::new(locals);
    for op in &raw.code {
        code(&mut f, op)?;
    }
    f.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&f);
    let mut module = Module::new();
    module.section(&types).section(&functions).section(&code);
    Some(module.finish())
}
pub fn valid(raw: &Raw) -> bool {
    encode(raw).is_some_and(|bytes| wasmparser::Validator::new().validate_all(&bytes).is_ok())
}
