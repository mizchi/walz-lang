//! Portable type shapes, Canonical ABI layouts, flattening and signatures.
use super::Adapter;
use crate::ir::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub(super) size: u32,
    pub(super) align: u32,
}

pub(super) fn type_key(ty: &Type) -> String {
    match ty {
        Type::Num(n) => n.wat().into(),
        Type::Value(id) => format!("v{}", id.0),
        Type::Unit => "unit".into(),
        _ => unreachable!(),
    }
}
pub(super) fn core_type(ty: &Type, module: &Module) -> String {
    match ty {
        Type::Num(n) => n.wat().into(),
        Type::Value(id) => format!("(ref {})", module.types[id.0].name),
        _ => unreachable!(),
    }
}
pub(super) fn pointer_at(ptr: &str, offset: u32) -> String {
    if offset == 0 {
        ptr.into()
    } else {
        format!("(i32.add {ptr} (i32.const {offset}))")
    }
}
pub(super) fn align(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) & !(alignment - 1)
}
pub(super) fn load(n: NumType, ptr: &str) -> String {
    format!("({}.load {ptr})", n.wat())
}
pub(super) fn component_type(ty: &Type) -> String {
    component_type_in(ty, "$")
}
pub(super) fn component_type_in(ty: &Type, scope: &str) -> String {
    match ty {
        Type::Num(NumType::I32) => "s32".into(),
        Type::Num(NumType::I64) => "s64".into(),
        Type::Num(NumType::V128) => unreachable!("v128 is not a portable ABI type"),
        Type::Num(n) => n.wat().into(),
        Type::Unit => format!("{scope}unit"),
        Type::Value(id) => format!("{scope}t{}", id.0),
        _ => unreachable!(),
    }
}

pub(super) fn coerce(value: &str, have: NumType, want: NumType) -> String {
    assert!(
        have != NumType::V128 && want != NumType::V128,
        "v128 is not a portable ABI type"
    );
    if have == want {
        return value.into();
    }
    let (bits, width) = match have {
        NumType::F32 => (format!("(i32.reinterpret_f32 {value})"), 32),
        NumType::F64 => (format!("(i64.reinterpret_f64 {value})"), 64),
        NumType::I32 => (value.into(), 32),
        NumType::I64 => (value.into(), 64),
        NumType::V128 => unreachable!("v128 is not a portable ABI type"),
    };
    let want_width = if matches!(want, NumType::I32 | NumType::F32) {
        32
    } else {
        64
    };
    let bits = match (width, want_width) {
        (32, 64) => format!("(i64.extend_i32_u {bits})"),
        (64, 32) => format!("(i32.wrap_i64 {bits})"),
        _ => bits,
    };
    match want {
        NumType::F32 => format!("(f32.reinterpret_i32 {bits})"),
        NumType::F64 => format!("(f64.reinterpret_i64 {bits})"),
        _ => bits,
    }
}

pub(super) fn component_signature(f: &Function, async_: bool, scope: &str) -> String {
    format!(
        "{}{}{}",
        if async_ { "async " } else { "" },
        f.params
            .iter()
            .enumerate()
            .map(|(i, id)| format!(
                "(param \"{}\" {}) ",
                f.import_contract
                    .as_ref()
                    .and_then(|b| b.param_names.get(i))
                    .cloned()
                    .unwrap_or_else(|| format!("p{i}")),
                component_type_in(&f.locals[id.0].ty, scope)
            ))
            .collect::<String>(),
        if f.result == Type::Unit {
            String::new()
        } else {
            format!("(result {})", component_type_in(&f.result, scope))
        }
    )
}
impl<'a> Adapter<'a> {
    pub(super) fn new(module: &'a Module) -> Self {
        let mut this = Self {
            module,
            endpoints: BTreeSet::new(),
            types: BTreeSet::from([Type::Unit]),
            memory_exports: false,
        };
        for num in [NumType::I32, NumType::I64, NumType::F32, NumType::F64] {
            this.types.insert(Type::Num(num));
        }
        for index in 0..module.types.len() {
            let ty = Type::Value(TypeId(index));
            if this.portable(&ty) {
                this.types.insert(ty);
                if matches!(
                    module.types[index].kind,
                    TypeKind::Value(ValueType::Future(_) | ValueType::Stream(_))
                ) {
                    this.endpoints.insert(TypeId(index));
                }
            }
        }
        this
    }
    pub(super) fn value(&self, id: TypeId) -> &ValueType {
        let TypeKind::Value(v) = &self.module.types[id.0].kind else {
            unreachable!()
        };
        v
    }
    fn portable(&self, ty: &Type) -> bool {
        match ty {
            Type::Num(NumType::V128) => false,
            Type::Unit | Type::Num(_) => true,
            Type::Value(id) => match &self.module.types[id.0].kind {
                TypeKind::Value(ValueType::Bytes | ValueType::Text) => true,
                TypeKind::Value(ValueType::Own(_)) => false,
                TypeKind::Value(
                    ValueType::List(t)
                    | ValueType::Option(t)
                    | ValueType::Future(t)
                    | ValueType::Stream(t),
                ) => self.portable(t),
                TypeKind::Value(ValueType::Result { ok, err }) => {
                    self.portable(ok) && self.portable(err)
                }
                TypeKind::Value(ValueType::Pair { first, second }) => {
                    self.portable(first) && self.portable(second)
                }
                _ => false,
            },
            _ => false,
        }
    }
    pub(super) fn layout(&self, ty: &Type) -> Layout {
        match ty {
            Type::Unit => Layout { size: 1, align: 1 },
            Type::Num(NumType::V128) => unreachable!("v128 is not a portable ABI type"),
            Type::Num(NumType::I64 | NumType::F64) => Layout { size: 8, align: 8 },
            Type::Num(_) => Layout { size: 4, align: 4 },
            Type::Value(id) => match self.value(*id) {
                ValueType::Bytes | ValueType::Text | ValueType::List(_) => {
                    Layout { size: 8, align: 4 }
                }
                ValueType::Pair { first, second } => {
                    let a = self.layout(first);
                    let b = self.layout(second);
                    let alignment = a.align.max(b.align);
                    Layout {
                        size: align(align(a.size, b.align) + b.size, alignment),
                        align: alignment,
                    }
                }
                ValueType::Future(_) | ValueType::Stream(_) => Layout { size: 4, align: 4 },
                _ => {
                    let cases = self.cases(*id);
                    let layouts = cases
                        .iter()
                        .map(|t| {
                            t.as_ref()
                                .map(|t| self.layout(t))
                                .unwrap_or(Layout { size: 0, align: 1 })
                        })
                        .collect::<Vec<_>>();
                    let alignment = layouts.iter().map(|x| x.align).max().unwrap_or(1);
                    Layout {
                        size: align(
                            align(1, alignment) + layouts.iter().map(|x| x.size).max().unwrap_or(0),
                            alignment,
                        ),
                        align: alignment,
                    }
                }
            },
            _ => unreachable!(),
        }
    }
    pub(super) fn cases(&self, id: TypeId) -> Vec<Option<Type>> {
        match self.value(id) {
            ValueType::Option(t) => vec![None, Some(t.clone())],
            ValueType::Result { ok, err } => vec![
                (*ok != Type::Unit).then(|| ok.clone()),
                (*err != Type::Unit).then(|| err.clone()),
            ],
            _ => unreachable!(),
        }
    }
    pub(super) fn flat(&self, ty: &Type) -> Vec<NumType> {
        match ty {
            Type::Num(n) => vec![*n],
            Type::Unit => vec![NumType::I32],
            Type::Value(id) => match self.value(*id) {
                ValueType::Bytes | ValueType::Text | ValueType::List(_) => {
                    vec![NumType::I32, NumType::I32]
                }
                ValueType::Future(_) | ValueType::Stream(_) => vec![NumType::I32],
                ValueType::Pair { first, second } => [self.flat(first), self.flat(second)].concat(),
                _ => {
                    let mut payload: Vec<NumType> = vec![];
                    for t in self.cases(*id).into_iter().flatten() {
                        for (i, n) in self.flat(&t).into_iter().enumerate() {
                            if i == payload.len() {
                                payload.push(n);
                            } else if payload[i] != n {
                                payload[i] = if matches!(
                                    (payload[i], n),
                                    (NumType::I32, NumType::F32) | (NumType::F32, NumType::I32)
                                ) {
                                    NumType::I32
                                } else {
                                    NumType::I64
                                };
                            }
                        }
                    }
                    let mut result = vec![NumType::I32];
                    result.extend(payload);
                    result
                }
            },
            _ => unreachable!(),
        }
    }
    pub(super) fn declarations(&self) -> String {
        let mut out = String::from(
            "(type $unit_raw (enum \"value\"))\n(export \"wlt-unit\" (type $unit (eq $unit_raw)))\n",
        );
        for ty in &self.types {
            let Type::Value(id) = ty else { continue };
            let def = match self.value(*id) {
                ValueType::Bytes => "(list u8)".into(),
                ValueType::Text => "string".into(),
                ValueType::List(t) => format!("(list {})", component_type(t)),
                ValueType::Option(t) => format!("(option {})", component_type(t)),
                ValueType::Result { ok, err } => format!(
                    "(result {}{})",
                    if *ok == Type::Unit {
                        String::new()
                    } else {
                        component_type(ok)
                    },
                    if *err == Type::Unit {
                        String::new()
                    } else {
                        format!(" (error {})", component_type(err))
                    }
                ),
                ValueType::Pair { first, second } => {
                    format!(
                        "(tuple {} {})",
                        component_type(first),
                        component_type(second)
                    )
                }
                ValueType::Future(t) | ValueType::Stream(t) => format!(
                    "({}{})",
                    if matches!(self.value(*id), ValueType::Future(_)) {
                        "future"
                    } else {
                        "stream"
                    },
                    if *t == Type::Unit {
                        String::new()
                    } else {
                        format!(" {}", component_type(t))
                    }
                ),
                _ => unreachable!(),
            };
            out.push_str(&format!(
                "(type $raw{} {def})\n(export \"t{}\" (type $t{} (eq $raw{})))\n",
                id.0, id.0, id.0, id.0
            ));
        }
        out
    }
}
