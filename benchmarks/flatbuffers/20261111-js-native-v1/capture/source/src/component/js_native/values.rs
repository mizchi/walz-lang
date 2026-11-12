//! Direct JS codecs and private GC constructors/accessors. No handle registry.
use super::{Mode, unsupported};
use crate::{
    Error,
    component::{Adapter, abi::type_key},
    interface::AbiType,
    ir::*,
};

pub(super) fn validate(a: &Adapter<'_>, ty: &Type) -> Result<(), Error> {
    let valid = match ty {
        Type::Unit | Type::Never => true,
        Type::Num(n) => *n != NumType::V128,
        Type::Newtype(_) => matches!(a.interface_type(ty), Some(AbiType::Bool)),
        Type::Ref {
            ty: id,
            nullable: false,
        } => {
            matches!(
                a.interface_type(ty),
                Some(AbiType::Record(_) | AbiType::Tuple)
            ) && matches!(&a.module.types[id.0].kind,TypeKind::Struct(fields) if fields.iter().all(|f|!f.mutable))
        }
        Type::Value(id) => matches!(
            a.value(*id),
            ValueType::Bytes
                | ValueType::Text
                | ValueType::List(_)
                | ValueType::Option(_)
                | ValueType::Result { .. }
                | ValueType::Pair { .. }
        ),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(unsupported(
            "boundary supports numeric scalars, bool, immutable ABI records/tuples, bytes, strings, lists, Option and Result",
        ))
    }
}

pub(super) struct Generator<'a> {
    a: &'a Adapter<'a>,
    mode: Mode,
    pub wat: String,
    pub js: String,
}
impl<'a> Generator<'a> {
    pub fn new(a: &'a Adapter<'a>, mode: Mode) -> Self {
        Self {
            a,
            mode,
            wat: String::new(),
            js: String::new(),
        }
    }
    fn wt(&self, t: &Type) -> String {
        crate::emit::ty(self.a.module, t)
    }
    fn export(&mut self, name: &str, params: &[Type], result: Option<&Type>, body: &str) {
        let p = params
            .iter()
            .enumerate()
            .map(|(i, t)| format!(" (param $p{i} {})", self.wt(t)))
            .collect::<String>();
        let r = result
            .map(|t| format!(" (result {})", self.wt(t)))
            .unwrap_or_default();
        self.wat += &format!("(func (export {name:?}){p}{r} {body})\n");
    }
    fn constructor(&mut self, key: &str, result: &Type, fields: &[Type], form: &str) {
        let args = (0..fields.len())
            .map(|i| format!(" (local.get $p{i})"))
            .collect::<String>();
        self.export(
            &format!("make_{key}"),
            fields,
            Some(result),
            &format!("({form}{args})"),
        );
    }
    fn variants(&mut self, ty: &Type, id: TypeId, cases: &[Variant]) {
        let key = type_key(ty);
        let base = &self.a.module.types[id.0].name;
        let mut tag = "(unreachable)".to_owned();
        for (i, case) in cases.iter().enumerate().rev() {
            let name = format!("$__variant_{}_{i}", id.0);
            tag = format!(
                "(if (result i32) (i32.eq (struct.get {base} 0 (local.get $p0)) (i32.const {i})) (then (drop (ref.cast (ref {name}) (local.get $p0))) (i32.const {i})) (else {tag}))"
            );
            self.constructor(
                &format!("{key}_{i}"),
                ty,
                &case.payloads,
                &format!("struct.new {name} (i32.const {i})"),
            );
            for (j, payload) in case.payloads.iter().enumerate() {
                self.export(
                    &format!("get_{key}_{i}_{j}"),
                    std::slice::from_ref(ty),
                    Some(payload),
                    &format!(
                        "(struct.get {name} {} (ref.cast (ref {name}) (local.get $p0)))",
                        j + 1
                    ),
                );
            }
        }
        self.export(
            &format!("tag_{key}"),
            std::slice::from_ref(ty),
            Some(&Type::Num(NumType::I32)),
            &tag,
        );
    }
    pub fn codec(&mut self, t: &Type) -> Result<(), Error> {
        let key = type_key(t);
        let (r, w) = match t {
            Type::Unit => (
                "return 'value';".into(),
                "if(v!=='value')throw new TypeError('expected unit');".into(),
            ),
            Type::Never => return Ok(()),
            Type::Num(n) => {
                let check = match n {
                    NumType::I32 => {
                        "if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');"
                    }
                    NumType::I64 => {
                        "if(typeof v!=='bigint'||v < -9223372036854775808n||v > 9223372036854775807n)throw new TypeError('expected s64');"
                    }
                    NumType::F32 | NumType::F64 => {
                        "if(typeof v!=='number')throw new TypeError('expected float');"
                    }
                    _ => return Err(unsupported("v128 boundary")),
                };
                ("return v;".into(), format!("{check}return v;"))
            }
            Type::Newtype(_) => (
                "if(v!==0&&v!==1)throw new TypeError('invalid bool');return !!v;".into(),
                "if(typeof v!=='boolean')throw new TypeError('expected bool');return v?1:0;".into(),
            ),
            Type::Ref { ty: id, .. } => {
                let TypeKind::Struct(fields) = &self.a.module.types[id.0].kind else {
                    unreachable!()
                };
                let fields = fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>();
                let name = &self.a.module.types[id.0].name;
                self.constructor(&key, t, &fields, &format!("struct.new {name}"));
                for (i, field) in fields.iter().enumerate() {
                    self.export(
                        &format!("get_{key}_{i}"),
                        std::slice::from_ref(t),
                        Some(field),
                        &format!("(struct.get {name} {i} (local.get $p0))"),
                    );
                }
                let names = match self.a.interface_type(t).unwrap() {
                    AbiType::Record(names) => Some(names),
                    AbiType::Tuple => None,
                    _ => unreachable!(),
                };
                let read = fields
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        let v = format!("r_{}(x.get_{key}_{i}(v))", type_key(field));
                        if let Some(names) = names {
                            format!(
                                "[{}]:{v}",
                                serde_json::to_string(&crate::bindings::js_name(&names[i]))
                                    .unwrap()
                            )
                        } else {
                            v
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let write = fields
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        format!(
                            "w_{}(v[{}])",
                            type_key(field),
                            names
                                .map(|n| serde_json::to_string(&crate::bindings::js_name(&n[i]))
                                    .unwrap())
                                .unwrap_or(i.to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let (shape, check) = if names.is_some() {
                    (format!("{{{read}}}"),"if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');".into())
                } else {
                    (
                        format!("[{read}]"),
                        format!(
                            "if(!Array.isArray(v)||v.length!=={})throw new TypeError('expected tuple');",
                            fields.len()
                        ),
                    )
                };
                (
                    format!("return {shape};"),
                    format!("{check}return x.make_{key}({write});"),
                )
            }
            Type::Value(id) => match self.a.value(*id).clone() {
                ValueType::Bytes | ValueType::Text => self.buffer(t, *id),
                ValueType::List(payload) => self.list(t, *id, &payload),
                value @ ValueType::Option(_)
                | value @ ValueType::Result { .. }
                | value @ ValueType::Pair { .. } => {
                    self.variants(t, *id, &value.variants(*id));
                    let read = |payload: &Type, case: usize, index: usize| {
                        if *payload == Type::Unit {
                            "r_unit()".into()
                        } else {
                            format!("r_{}(x.get_{key}_{case}_{index}(v))", type_key(payload))
                        }
                    };
                    let write = |payload: &Type, v: &str| format!("w_{}({v})", type_key(payload));
                    let make = |case: usize, payload: &Type, v: &str| {
                        if *payload == Type::Unit {
                            format!("w_unit({v});return x.make_{key}_{case}();")
                        } else {
                            format!("return x.make_{key}_{case}({});", write(payload, v))
                        }
                    };
                    match value {
                        ValueType::Option(payload) => {
                            let nested = matches!(&payload,Type::Value(i) if matches!(self.a.value(*i),ValueType::Option(_)));
                            let none = if nested { "{tag:'none'}" } else { "undefined" };
                            let some = read(&payload, 1, 0);
                            let some = if nested {
                                format!("{{tag:'some',val:{some}}}")
                            } else {
                                some
                            };
                            (
                                format!("return x.tag_{key}(v)===0?{none}:{some};"),
                                format!(
                                    "{}if({})return x.make_{key}_0();{}",
                                    if nested {
                                        "if(v?.tag!=='none'&&v?.tag!=='some')throw new TypeError('expected option');"
                                    } else {
                                        ""
                                    },
                                    if nested {
                                        "v.tag==='none'"
                                    } else {
                                        "v===undefined"
                                    },
                                    make(1, &payload, if nested { "v.val" } else { "v" })
                                ),
                            )
                        }
                        ValueType::Result { ok, err } => {
                            let r = |p: &Type, i| {
                                if *p == Type::Unit {
                                    "undefined".into()
                                } else {
                                    read(p, i, 0)
                                }
                            };
                            let w = |p: &Type, i| {
                                if *p == Type::Unit {
                                    format!("return x.make_{key}_{i}();")
                                } else {
                                    make(i, p, "v.val")
                                }
                            };
                            (
                                format!(
                                    "return x.tag_{key}(v)===0?{{tag:'ok',val:{}}}:{{tag:'err',val:{}}};",
                                    r(&ok, 0),
                                    r(&err, 1)
                                ),
                                format!(
                                    "if(v?.tag==='ok'){{{}}}if(v?.tag==='err'){{{}}}throw new TypeError('expected result');",
                                    w(&ok, 0),
                                    w(&err, 1)
                                ),
                            )
                        }
                        ValueType::Pair { first, second } => {
                            let j = usize::from(first != Type::Unit);
                            let mut args = Vec::new();
                            let mut checks = String::new();
                            for (i, payload) in [&first, &second].iter().enumerate() {
                                if **payload == Type::Unit {
                                    checks += &format!("w_unit(v[{i}]);");
                                } else {
                                    args.push(write(payload, &format!("v[{i}]")));
                                }
                            }
                            (
                                format!(
                                    "x.tag_{key}(v);return [{},{}];",
                                    read(&first, 0, 0),
                                    read(&second, 0, j)
                                ),
                                format!(
                                    "if(!Array.isArray(v)||v.length!==2)throw new TypeError('expected pair');{checks}return x.make_{key}_0({});",
                                    args.join(",")
                                ),
                            )
                        }
                        _ => unreachable!(),
                    }
                }
                _ => return Err(unsupported("owned boundary")),
            },
            _ => return Err(unsupported("boundary type")),
        };
        self.js += &format!("function r_{key}(v){{{r}}}\nfunction w_{key}(v){{{w}}}\n");
        Ok(())
    }
    fn buffer(&mut self, t: &Type, id: TypeId) -> (String, String) {
        let key = type_key(t);
        let name = &self.a.module.types[id.0].name;
        let text = matches!(self.a.value(id), ValueType::Text);
        if text && self.mode == Mode::JsStrings {
            self.wat += &format!(
                "(func (export \"make_{key}\") (param externref) (result (ref {name})) (struct.new {name} (call $__js_string_cast (local.get 0)) (ref.null $__byte_storage)))\n(func (export \"get_{key}\") (param (ref {name})) (result externref) (struct.get {name} 0 (local.get 0)))\n"
            );
            (
                format!("return x.get_{key}(v);"),
                format!(
                    "if(typeof v!=='string')throw new TypeError('expected string');return x.make_{key}(c.wellFormed(v));"
                ),
            )
        } else {
            self.wat += &format!(
                "(func (export \"make_{key}\") (param i32) (result (ref {name})) (struct.new {name} (call $__native_from_memory (local.get 0))))\n(func (export \"get_{key}\") (param (ref {name})) (result i32) (call $__native_to_memory (struct.get {name} 0 (local.get 0))))\n"
            );
            (
                format!(
                    "return c.{}(x.get_{key}(v));",
                    if text { "decode" } else { "readBytes" }
                ),
                format!(
                    "return x.make_{key}(c.{}(v));",
                    if text { "writeText" } else { "writeBytes" }
                ),
            )
        }
    }
    fn list(&mut self, t: &Type, id: TypeId, payload: &Type) -> (String, String) {
        let key = type_key(t);
        let name = &self.a.module.types[id.0].name;
        let storage = format!("$__list_storage_{}", id.0);
        self.export(
            &format!("len_{key}"),
            std::slice::from_ref(t),
            Some(&Type::Num(NumType::I32)),
            &format!("(array.len (struct.get {name} 0 (local.get $p0)))"),
        );
        let limit = super::super::MEMORY_LIMIT / self.a.layout(payload).size.max(1);
        if let Type::Num(n) = payload {
            let (array, width) = match n {
                NumType::I32 => ("Int32Array", 4),
                NumType::I64 => ("BigInt64Array", 8),
                NumType::F32 => ("Float32Array", 4),
                NumType::F64 => ("Float64Array", 8),
                _ => unreachable!(),
            };
            let num = n.wat();
            let result = self.wt(t);
            self.wat += &format!(
                r#"
              (func (export "make_{key}") (param $n i32) (result {result})
                (local $a (ref {storage})) (local $i i32)
                (if (i32.gt_u (local.get $n) (i32.const {limit})) (then unreachable))
                (local.set $a (array.new_default {storage} (local.get $n)))
                (loop $copy (if (i32.lt_u (local.get $i) (local.get $n)) (then
                  (array.set {storage} (local.get $a) (local.get $i) ({num}.load (i32.mul (local.get $i) (i32.const {width}))))
                  (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
                (struct.new {name} (local.get $a)))
              (func (export "get_{key}") (param $v {result}) (result i32)
                (local $a (ref {storage})) (local $i i32) (local $n i32)
                (local.set $a (struct.get {name} 0 (local.get $v))) (local.set $n (array.len (local.get $a)))
                (if (i32.gt_u (local.get $n) (i32.const {limit})) (then unreachable))
                (call $__native_reserve (i32.mul (local.get $n) (i32.const {width})))
                (loop $copy (if (i32.lt_u (local.get $i) (local.get $n)) (then
                  ({num}.store (i32.mul (local.get $i) (i32.const {width})) (array.get {storage} (local.get $a) (local.get $i)))
                  (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
                (local.get $n))
            "#
            );
            let kind = match n {
                NumType::I32 => "Int32",
                NumType::I64 => "BigInt64",
                NumType::F32 => "Float32",
                NumType::F64 => "Float64",
                _ => unreachable!(),
            };
            (
                format!("return c.readList(x.get_{key}(v),{array},{width},'{kind}');"),
                format!(
                    "const n=c.writeList(v,{array},'{array}',{width},'{kind}',w_{});return x.make_{key}(n);",
                    type_key(payload)
                ),
            )
        } else {
            self.export(
                &format!("empty_{key}"),
                &[],
                Some(t),
                &format!("(struct.new {name} (array.new_fixed {storage} 0))"),
            );
            let result = self.wt(t);
            if *payload == Type::Unit {
                self.wat += &format!(
                    "(func (export \"make_{key}\") (param $n i32) (result {result}) (struct.new {name} (array.new_default {storage} (local.get $n))))\n"
                );
                (
                    format!(
                        "const n=x.len_{key}(v);if(n>{limit})throw new RangeError('list limit');return Array.from({{length:n}},()=>r_unit());"
                    ),
                    format!(
                        "const n=c.arrayLength(v,{limit});for(let i=0;i<n;i++)w_unit(v[i]);return x.make_{key}(n);"
                    ),
                )
            } else {
                self.export(
                    &format!("make_{key}"),
                    &[payload.clone(), Type::Num(NumType::I32)],
                    Some(t),
                    &format!(
                        "(struct.new {name} (array.new {storage} (local.get $p0) (local.get $p1)))"
                    ),
                );
                self.export(&format!("set_{key}"),&[t.clone(),Type::Num(NumType::I32),payload.clone()],None,&format!("(array.set {storage} (struct.get {name} 0 (local.get $p0)) (local.get $p1) (local.get $p2))"));
                self.export(&format!("get_{key}"),&[t.clone(),Type::Num(NumType::I32)],Some(payload),&format!("(array.get {storage} (struct.get {name} 0 (local.get $p0)) (local.get $p1))"));
                (
                    format!(
                        "const n=x.len_{key}(v);if(n>{limit})throw new RangeError('list limit');return Array.from({{length:n}},(_,i)=>r_{}(x.get_{key}(v,i)));",
                        type_key(payload)
                    ),
                    format!(
                        "const n=c.arrayLength(v,{limit});if(!n)return x.empty_{key}();const a=x.make_{key}(w_{}(v[0]),n);for(let i=1;i<n;i++)x.set_{key}(a,i,w_{}(v[i]));return a;",
                        type_key(payload),
                        type_key(payload)
                    ),
                )
            }
        }
    }
}
