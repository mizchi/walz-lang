//! Generate only the concrete boundary value conversions used by this module.
use super::super::Adapter;
use super::super::abi::{align, type_key};
use crate::ir::*;

pub(super) fn read(ty: &Type, p: &str) -> String {
    format!("r_{}({p})", type_key(ty))
}
pub(super) fn write(ty: &Type, p: &str, v: &str) -> String {
    format!("w_{}({p},{v});", type_key(ty))
}

pub(super) fn codec(a: &Adapter<'_>, ty: &Type) -> (String, String) {
    let view = "c.view()";
    match ty {
        Type::Num(n) => {
            let (method, check) = match n {
                NumType::I32 => (
                    "Int32",
                    "if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');",
                ),
                NumType::I64 => (
                    "BigInt64",
                    "if(typeof v!=='bigint'||v < -9223372036854775808n||v > 9223372036854775807n)throw new TypeError('expected s64');",
                ),
                NumType::F32 => (
                    "Float32",
                    "if(typeof v!=='number')throw new TypeError('expected f32');",
                ),
                NumType::F64 => (
                    "Float64",
                    "if(typeof v!=='number')throw new TypeError('expected f64');",
                ),
            };
            (
                format!("return {view}.get{method}(p,true);"),
                format!("{check}{view}.set{method}(p,v,true);"),
            )
        }
        Type::Unit => (
            "if(c.view().getUint8(p)!==0)throw new TypeError('invalid unit');return 'value';"
                .into(),
            "if(v!=='value')throw new TypeError('expected unit');c.view().setUint8(p,0);".into(),
        ),
        Type::Value(id) => match a.value(*id) {
            ValueType::Bytes | ValueType::Text => {
                let text = matches!(a.value(*id), ValueType::Text);
                let r = if text {
                    "return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);"
                } else {
                    "return bytes;"
                };
                let w = if text {
                    "if(typeof v!=='string')throw new TypeError('expected string');const bytes=new TextEncoder().encode(v);"
                } else {
                    "if(!(v instanceof Uint8Array))throw new TypeError('expected Uint8Array');const bytes=v;"
                };
                (
                    format!(
                        "const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();{r}"
                    ),
                    format!(
                        "{w}const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);"
                    ),
                )
            }
            ValueType::Future(_) | ValueType::Stream(_) => (
                format!("return c.take({},c.view().getUint32(p,true));", id.0),
                format!("c.view().setUint32(p,c.add({},v),true);", id.0),
            ),
            ValueType::Pair { first, second } => {
                let p = format!("p+{}", align(a.layout(first).size, a.layout(second).align));
                (
                    format!("return [{},{}];", read(first, "p"), read(second, &p)),
                    format!(
                        "if(!Array.isArray(v)||v.length!==2)throw new TypeError('expected pair');{}{}",
                        write(first, "p", "v[0]"),
                        write(second, &p, "v[1]")
                    ),
                )
            }
            ValueType::Option(payload) => {
                let p = format!("p+{}", align(1, a.layout(ty).align));
                let nested = matches!(payload,Type::Value(id) if matches!(a.value(*id),ValueType::Option(_)));
                let (none, some, is_none, value) = if nested {
                    (
                        "{tag:'none'}",
                        format!("{{tag:'some',val:{}}}", read(payload, &p)),
                        "v?.tag==='none'",
                        "v.val",
                    )
                } else {
                    ("undefined", read(payload, &p), "v===undefined", "v")
                };
                let check = if nested {
                    "if(v?.tag!=='none'&&v?.tag!=='some')throw new TypeError('expected option');"
                } else {
                    ""
                };
                (
                    format!(
                        "const tag=c.view().getUint8(p);if(tag===0)return {none};if(tag!==1)throw new TypeError('invalid option');return {some};"
                    ),
                    format!(
                        "{check}if({is_none})c.view().setUint8(p,0);else{{c.view().setUint8(p,1);{}}}",
                        write(payload, &p, value)
                    ),
                )
            }
            ValueType::Result { ok, err } => {
                let p = format!("p+{}", align(1, a.layout(ty).align));
                let r = |t: &Type| {
                    if *t == Type::Unit {
                        "undefined".into()
                    } else {
                        read(t, &p)
                    }
                };
                let w = |t: &Type| {
                    if *t == Type::Unit {
                        String::new()
                    } else {
                        write(t, &p, "v.val")
                    }
                };
                (
                    format!(
                        "const tag=c.view().getUint8(p);if(tag===0)return {{tag:'ok',val:{}}};if(tag===1)return {{tag:'err',val:{}}};throw new TypeError('invalid result');",
                        r(ok),
                        r(err)
                    ),
                    format!(
                        "if(v?.tag==='ok'){{c.view().setUint8(p,0);{}}}else if(v?.tag==='err'){{c.view().setUint8(p,1);{}}}else throw new TypeError('expected result');",
                        w(ok),
                        w(err)
                    ),
                )
            }
            _ => unreachable!("portable types only"),
        },
        _ => unreachable!("portable types only"),
    }
}
