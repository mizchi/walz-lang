//! Canonical ABI for explicitly annotated immutable records and nominal scalars.
use super::{
    Adapter,
    abi::{Layout, align, component_type, core_type, pointer_at, type_key},
};
use crate::{interface::AbiType, ir::*};

impl Adapter<'_> {
    pub(super) fn ordered_types(&self) -> Vec<Type> {
        fn visit(
            a: &Adapter<'_>,
            t: &Type,
            seen: &mut std::collections::BTreeSet<Type>,
            out: &mut Vec<Type>,
        ) {
            if !seen.insert(t.clone()) {
                return;
            }
            if matches!(
                a.interface_type(t),
                Some(AbiType::Record(_) | AbiType::Tuple)
            ) {
                for (_, field, _) in a.fields(t) {
                    visit(a, &field, seen, out);
                }
            } else if matches!(a.interface_type(t), Some(AbiType::Variant(_))) {
                for payload in a.variant_payloads(t).into_iter().flatten() {
                    visit(a, &payload, seen, out);
                }
            } else if let Type::Value(id) = t {
                match a.value(*id) {
                    ValueType::List(t)
                    | ValueType::Option(t)
                    | ValueType::Future(t)
                    | ValueType::Stream(t) => visit(a, t, seen, out),
                    ValueType::Pair { first, second } => {
                        visit(a, first, seen, out);
                        visit(a, second, seen, out);
                    }
                    ValueType::Result { ok, err } => {
                        visit(a, ok, seen, out);
                        visit(a, err, seen, out);
                    }
                    _ => {}
                }
            }
            out.push(t.clone());
        }
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for t in &self.types {
            visit(self, t, &mut seen, &mut out);
        }
        out
    }
    pub(super) fn interface_id(&self, ty: &Type) -> Option<TypeId> {
        match ty {
            Type::Ref {
                ty: id,
                nullable: false,
            }
            | Type::Newtype(id)
                if self.module.types[id.0].abi.is_some() =>
            {
                Some(*id)
            }
            _ => None,
        }
    }
    pub(super) fn interface_type(&self, ty: &Type) -> Option<&AbiType> {
        self.module.types[self.interface_id(ty)?.0].abi.as_ref()
    }
    fn fields(&self, ty: &Type) -> Vec<(usize, Type, u32)> {
        let id = self.interface_id(ty).unwrap();
        let TypeKind::Struct(fields) = &self.module.types[id.0].kind else {
            unreachable!()
        };
        let mut offset = 0;
        fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let layout = self.layout(&f.ty);
                offset = align(offset, layout.align);
                let p = offset;
                offset += layout.size;
                (i, f.ty.clone(), p)
            })
            .collect()
    }
    fn width(&self, ty: &Type) -> u32 {
        match self.interface_type(ty).unwrap() {
            AbiType::Bool | AbiType::S8 | AbiType::U8 => 1,
            AbiType::S16 => 2,
            AbiType::Char => 4,
            AbiType::U16 => 2,
            AbiType::U32 => 4,
            AbiType::U64 => 8,
            AbiType::Enum(names) => {
                if names.len() <= 256 {
                    1
                } else if names.len() <= 65536 {
                    2
                } else {
                    4
                }
            }
            AbiType::Flags(names) => {
                if names.len() <= 8 {
                    1
                } else if names.len() <= 16 {
                    2
                } else {
                    4
                }
            }
            _ => unreachable!(),
        }
    }
    pub(super) fn interface_layout(&self, ty: &Type) -> Layout {
        if matches!(self.interface_type(ty), Some(AbiType::Variant(_))) {
            return self.variant_layout(ty);
        }
        if matches!(
            self.interface_type(ty),
            Some(AbiType::Record(_) | AbiType::Tuple)
        ) {
            let fields = self.fields(ty);
            let alignment = fields
                .iter()
                .map(|(_, t, _)| self.layout(t).align)
                .max()
                .unwrap_or(1);
            Layout {
                size: align(
                    fields.last().map_or(0, |(_, t, p)| p + self.layout(t).size),
                    alignment,
                ),
                align: alignment,
            }
        } else {
            let size = self.width(ty);
            Layout {
                size,
                align: if matches!(self.interface_type(ty), Some(AbiType::Flags(_))) {
                    size.min(4)
                } else {
                    size
                },
            }
        }
    }
    pub(super) fn interface_flat(&self, ty: &Type) -> Vec<NumType> {
        match self.interface_type(ty).unwrap() {
            AbiType::Variant(_) => self.variant_flat(ty),
            AbiType::Record(_) | AbiType::Tuple => self
                .fields(ty)
                .iter()
                .flat_map(|(_, t, _)| self.flat(t))
                .collect(),
            AbiType::U64 => vec![NumType::I64],
            AbiType::Flags(names) if names.len() > 32 => vec![NumType::I32, NumType::I32],
            _ => vec![NumType::I32],
        }
    }
    pub(super) fn interface_declaration(&self, ty: &Type) -> String {
        let quoted = |s: &str| serde_json::to_string(s).unwrap();
        match self.interface_type(ty).unwrap() {
            AbiType::Bool => "bool".into(),
            AbiType::Char => "char".into(),
            AbiType::S8 => "s8".into(),
            AbiType::S16 => "s16".into(),
            AbiType::Tuple => format!(
                "(tuple {})",
                self.fields(ty)
                    .iter()
                    .map(|(_, t, _)| component_type(t))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            AbiType::Variant(names) => format!(
                "(variant {})",
                names
                    .iter()
                    .zip(self.variant_payloads(ty))
                    .map(|(n, t)| format!(
                        "(case {}{})",
                        quoted(n),
                        t.map(|t| format!(" {}", component_type(&t)))
                            .unwrap_or_default()
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            AbiType::U8 => "u8".into(),
            AbiType::U16 => "u16".into(),
            AbiType::U32 => "u32".into(),
            AbiType::U64 => "u64".into(),
            AbiType::Enum(names) | AbiType::Flags(names) => format!(
                "({} {})",
                if matches!(self.interface_type(ty), Some(AbiType::Enum(_))) {
                    "enum"
                } else {
                    "flags"
                },
                names
                    .iter()
                    .map(|n| quoted(n))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            AbiType::Record(names) => format!(
                "(record {})",
                self.fields(ty)
                    .iter()
                    .zip(names)
                    .map(|((_, t, _), name)| format!(
                        "(field {} {})",
                        quoted(name),
                        component_type(t)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        }
    }
    fn enum_lift(&self, ty: &Type, tag: &str) -> String {
        let id = self.interface_id(ty).unwrap();
        let AbiType::Enum(names) = self.interface_type(ty).unwrap() else {
            unreachable!()
        };
        let mut out = "(unreachable)".to_owned();
        for i in (0..names.len()).rev() {
            out = format!(
                "(if (result {}) (i32.eq {tag} (i32.const {i})) (then (struct.new $__variant_{}_{i} (i32.const {i}))) (else {out}))",
                core_type(ty, self.module),
                id.0
            );
        }
        out
    }
    fn scalar_load(&self, ty: &Type, ptr: &str) -> String {
        if matches!(self.interface_type(ty), Some(AbiType::S8)) {
            return format!("(i32.load8_s {ptr})");
        }
        if matches!(self.interface_type(ty), Some(AbiType::S16)) {
            return format!("(i32.load16_s {ptr})");
        }
        match self.width(ty) {
            1 => format!("(i32.load8_u {ptr})"),
            2 => format!("(i32.load16_u {ptr})"),
            4 => format!("(i32.load {ptr})"),
            _ => format!("(i64.load {ptr})"),
        }
    }
    fn flag_check(&self, ty: &Type, bits: &str) -> String {
        match self.interface_type(ty) {
            Some(AbiType::Bool) => {
                return format!("(if (i32.gt_u {bits} (i32.const 1)) (then (unreachable)))");
            }
            Some(AbiType::Char) => {
                return format!(
                    "(if (i32.or (i32.gt_u {bits} (i32.const 1114111)) (i32.and (i32.ge_u {bits} (i32.const 55296)) (i32.le_u {bits} (i32.const 57343)))) (then (unreachable)))"
                );
            }
            Some(AbiType::S8 | AbiType::S16) => {
                let n = self.width(ty) * 8;
                let max = (1i32 << (n - 1)) - 1;
                let min = -(1i32 << (n - 1));
                return format!(
                    "(if (i32.or (i32.lt_s {bits} (i32.const {min})) (i32.gt_s {bits} (i32.const {max}))) (then (unreachable)))"
                );
            }
            _ => {}
        }
        if matches!(self.interface_type(ty), Some(AbiType::U8 | AbiType::U16)) {
            let limit = 1u32 << (self.width(ty) * 8);
            return format!("(if (i32.ge_u {bits} (i32.const {limit})) (then (unreachable)))");
        }
        let AbiType::Flags(names) = self.interface_type(ty).unwrap() else {
            return String::new();
        };
        let mask = 1u64
            .checked_shl(names.len() as u32)
            .unwrap_or(0)
            .wrapping_sub(1);
        let (num, invalid) = if names.len() <= 32 {
            ("i32", u64::from(!(mask as u32)))
        } else {
            ("i64", !mask)
        };
        format!(
            "(if ({num}.ne ({num}.and {bits} ({num}.const {invalid})) ({num}.const 0)) (then (unreachable)))"
        )
    }
    pub(super) fn interface_lift(&self, ty: &Type, ptr: &str) -> String {
        match self.interface_type(ty).unwrap() {
            AbiType::Variant(_) => self.variant_lift(ty, ptr),
            AbiType::Record(_) | AbiType::Tuple => format!(
                "(struct.new {} {})",
                self.module.types[self.interface_id(ty).unwrap().0].name,
                self.fields(ty)
                    .iter()
                    .map(|(_, t, p)| self.lift(t, &pointer_at(ptr, *p)))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            AbiType::Enum(_) => self.enum_lift(ty, &self.scalar_load(ty, ptr)),
            _ => {
                let value = self.scalar_load(ty, ptr);
                format!(
                    "(block (result {}) {} {value})",
                    core_type(ty, self.module),
                    self.flag_check(ty, &value)
                )
            }
        }
    }
    pub(super) fn interface_lower(&self, ty: &Type, value: &str, ptr: &str) -> String {
        let id = self.interface_id(ty).unwrap();
        match self.interface_type(ty).unwrap() {
            AbiType::Variant(_) => self.variant_lower(ty, value, ptr),
            AbiType::Record(_) | AbiType::Tuple => format!(
                "(block {})",
                self.fields(ty)
                    .iter()
                    .map(|(i, t, p)| self.lower(
                        t,
                        &format!("(struct.get {} {i} {value})", self.module.types[id.0].name),
                        &pointer_at(ptr, *p)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            AbiType::Enum(names) => {
                let tag = format!("(struct.get {} 0 {value})", self.module.types[id.0].name);
                let mut check = "(unreachable)".to_owned();
                for i in (0..names.len()).rev() {
                    check = format!(
                        "(if (i32.eq {tag} (i32.const {i})) (then (drop (ref.cast (ref $__variant_{}_{i}) {value}))) (else {check}))",
                        id.0
                    );
                }
                let store = match self.width(ty) {
                    1 => "i32.store8",
                    2 => "i32.store16",
                    _ => "i32.store",
                };
                format!("(block {check} ({store} {ptr} {tag}))")
            }
            _ => {
                let store = match self.width(ty) {
                    1 => "i32.store8",
                    2 => "i32.store16",
                    4 => "i32.store",
                    _ => "i64.store",
                };
                format!(
                    "(block {} ({store} {ptr} {value}))",
                    self.flag_check(ty, value)
                )
            }
        }
    }
    pub(super) fn interface_release(&self, ty: &Type, ptr: &str) -> String {
        if matches!(self.interface_type(ty), Some(AbiType::Variant(_))) {
            return self.variant_release(ty, ptr);
        }
        if matches!(
            self.interface_type(ty),
            Some(AbiType::Record(_) | AbiType::Tuple)
        ) {
            format!(
                "(block {})",
                self.fields(ty)
                    .iter()
                    .map(|(_, t, p)| self.release(t, &pointer_at(ptr, *p)))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        } else {
            "(nop)".into()
        }
    }
    pub(super) fn interface_flat_load(&self, ty: &Type, ptr: &str) -> Vec<(String, NumType)> {
        match self.interface_type(ty).unwrap() {
            AbiType::Variant(_) => self.variant_flat_load(ty, ptr),
            AbiType::Record(_) | AbiType::Tuple => self
                .fields(ty)
                .iter()
                .flat_map(|(_, t, p)| self.flat_load(t, &pointer_at(ptr, *p)))
                .collect(),
            AbiType::Flags(names) if names.len() > 32 => vec![
                (format!("(i32.load {ptr})"), NumType::I32),
                (format!("(i32.load {})", pointer_at(ptr, 4)), NumType::I32),
            ],
            _ => vec![(
                self.scalar_load(ty, ptr),
                if matches!(self.interface_type(ty), Some(AbiType::U64)) {
                    NumType::I64
                } else {
                    NumType::I32
                },
            )],
        }
    }
    pub(super) fn interface_flat_lift(&self, ty: &Type, values: &[(String, NumType)]) -> String {
        match self.interface_type(ty).unwrap() {
            AbiType::Variant(_) => self.variant_flat_lift(ty, values),
            AbiType::Record(_) | AbiType::Tuple => {
                let mut index = 0;
                let fields = self
                    .fields(ty)
                    .iter()
                    .map(|(_, t, _)| {
                        let n = self.flat(t).len();
                        let s = self.flat_lift(t, &values[index..index + n]);
                        index += n;
                        s
                    })
                    .collect::<Vec<_>>();
                format!(
                    "(struct.new {} {})",
                    self.module.types[self.interface_id(ty).unwrap().0].name,
                    fields.join(" ")
                )
            }
            AbiType::Enum(_) => self.enum_lift(ty, &values[0].0),
            AbiType::Flags(names) if names.len() > 32 => {
                let value = format!(
                    "(i64.or (i64.extend_i32_u {}) (i64.shl (i64.extend_i32_u {}) (i64.const 32)))",
                    values[0].0, values[1].0
                );
                format!(
                    "(block (result i64) {} {value})",
                    self.flag_check(ty, &value)
                )
            }
            _ => format!(
                "(block (result {}) {} {})",
                core_type(ty, self.module),
                self.flag_check(ty, &values[0].0),
                values[0].0
            ),
        }
    }
    pub(super) fn interface_codec(&self, ty: &Type) -> (String, String) {
        let q = |s: &str| serde_json::to_string(s).unwrap();
        match self.interface_type(ty).unwrap() {
            AbiType::Bool=>("const n=c.view().getUint8(p);if(n>1)throw new TypeError('invalid bool');return !!n;".into(),"if(typeof v!=='boolean')throw new TypeError('expected bool');c.view().setUint8(p,v?1:0);".into()),
            AbiType::Char=>("const n=c.view().getUint32(p,true);if(n>0x10ffff||(n>=0xd800&&n<=0xdfff))throw new TypeError('invalid char');return String.fromCodePoint(n);".into(),"if(typeof v!=='string'||[...v].length!==1)throw new TypeError('expected char');const n=v.codePointAt(0);if(n>=0xd800&&n<=0xdfff)throw new TypeError('invalid char');c.view().setUint32(p,n,true);".into()),
            AbiType::S8|AbiType::S16=>{let bits=self.width(ty)*8;let max=(1i32<<(bits-1))-1;let min=-(1i32<<(bits-1));(format!("return c.view().getInt{bits}(p,true);"),format!("if(!Number.isInteger(v)||v<{min}||v>{max})throw new TypeError('expected s{bits}');c.view().setInt{bits}(p,v,true);"))},
            AbiType::Tuple=>{let fields=self.fields(ty);let read=fields.iter().map(|(_,t,p)|format!("r_{}(p+{p})",type_key(t))).collect::<Vec<_>>().join(",");let write=fields.iter().map(|(i,t,p)|format!("w_{}(p+{p},v[{i}]);",type_key(t))).collect::<String>();(format!("return [{read}];"),format!("if(!Array.isArray(v)||v.length!=={})throw new TypeError('expected tuple');{write}",fields.len()))},
            AbiType::Variant(names)=>self.variant_codec(ty,names),

            AbiType::U8|AbiType::U16|AbiType::U32=>{
                let bits=self.width(ty)*8;
                let max=(1u64<<bits)-1;
                (format!("return c.view().getUint{bits}(p,true);"),format!("if(!Number.isInteger(v)||v<0||v>{max})throw new TypeError('expected u{bits}');c.view().setUint{bits}(p,v,true);"))
            },
            AbiType::U64=>("return c.view().getBigUint64(p,true);".into(),"if(typeof v!=='bigint'||v<0n||v>18446744073709551615n)throw new TypeError('expected u64');c.view().setBigUint64(p,v,true);".into()),
            AbiType::Record(names)=>{
                let fields=self.fields(ty);
                let read=fields.iter().zip(names).map(|((_,t,p),name)|format!("{}:r_{}(p+{p})",q(&crate::bindings::js_name(name)),type_key(t))).collect::<Vec<_>>().join(",");
                let write=fields.iter().zip(names).map(|((_,t,p),name)|format!("w_{}(p+{p},v[{}]);",type_key(t),q(&crate::bindings::js_name(name)))).collect::<String>();
                (format!("return {{{read}}};"),format!("if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');{write}"))
            }
            AbiType::Enum(names)=>{
                let width=self.width(ty)*8;
                let names=serde_json::to_string(names).unwrap();
                (format!("const n=c.view().getUint{width}(p,true),names={names};if(n>=names.length)throw new TypeError('invalid enum');return names[n];"),format!("const n={names}.indexOf(v);if(n<0)throw new TypeError('expected enum case');c.view().setUint{width}(p,n,true);"))
            }
            AbiType::Flags(names)=>{
                let width=self.width(ty)*8;
                let method=if width==64 {"BigUint64".into()}else{format!("Uint{width}")};
                let read=names.iter().enumerate().map(|(i,n)|format!("{}:!!(bits&(1n<<{i}n))",q(&crate::bindings::js_name(n)))).collect::<Vec<_>>().join(",");
                let write=names.iter().enumerate().map(|(i,n)|{let key=q(&crate::bindings::js_name(n));format!("if(v[{key}]!==undefined&&typeof v[{key}]!=='boolean')throw new TypeError('expected flag boolean');if(v[{key}])bits|=1n<<{i}n;")}).collect::<String>();
                (format!("const bits=BigInt(c.view().get{method}(p,true));return {{{read}}};"),format!("if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected flags');let bits=0n;{write}c.view().set{method}(p,{},true);",if width==64 {"bits"} else {"Number(bits)"}))
            }
        }
    }
}
