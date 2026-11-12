//! Canonical discriminants and joined flat payloads for named WIT variants.
use super::{
    Adapter,
    abi::{Layout, align, coerce, core_type, pointer_at, type_key},
};
use crate::ir::*;

fn select(tag: &str, result: Option<&str>, cases: &[String]) -> String {
    let result = result.map(|t| format!(" (result {t})")).unwrap_or_default();
    cases
        .iter()
        .enumerate()
        .rev()
        .fold("(unreachable)".into(), |tail, (i, body)| {
            format!("(if{result} (i32.eq {tag} (i32.const {i})) (then {body}) (else {tail}))")
        })
}
impl Adapter<'_> {
    pub(super) fn variant_payloads(&self, ty: &Type) -> Vec<Option<Type>> {
        let TypeKind::Variant(cases) = &self.module.types[self.interface_id(ty).unwrap().0].kind
        else {
            unreachable!()
        };
        cases.iter().map(|c| c.payloads.first().cloned()).collect()
    }
    fn variant_tag_width(&self, ty: &Type) -> u32 {
        match self.variant_payloads(ty).len() {
            0..=256 => 1,
            257..=65536 => 2,
            _ => 4,
        }
    }
    fn variant_offset(&self, ty: &Type) -> u32 {
        align(self.variant_tag_width(ty), self.variant_layout(ty).align)
    }
    fn variant_tag(&self, ty: &Type, ptr: &str) -> String {
        match self.variant_tag_width(ty) {
            1 => format!("(i32.load8_u {ptr})"),
            2 => format!("(i32.load16_u {ptr})"),
            _ => format!("(i32.load {ptr})"),
        }
    }
    pub(super) fn variant_layout(&self, ty: &Type) -> Layout {
        let layouts = self
            .variant_payloads(ty)
            .into_iter()
            .flatten()
            .map(|t| self.layout(&t))
            .collect::<Vec<_>>();
        let alignment = layouts
            .iter()
            .map(|l| l.align)
            .max()
            .unwrap_or(1)
            .max(self.variant_tag_width(ty));
        Layout {
            align: alignment,
            size: align(
                align(self.variant_tag_width(ty), alignment)
                    + layouts.iter().map(|l| l.size).max().unwrap_or(0),
                alignment,
            ),
        }
    }
    pub(super) fn variant_flat(&self, ty: &Type) -> Vec<NumType> {
        let mut payload = Vec::new();
        for t in self.variant_payloads(ty).into_iter().flatten() {
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
        [vec![NumType::I32], payload].concat()
    }
    fn variant_construct(&self, ty: &Type, index: usize, payload: &str) -> String {
        format!(
            "(struct.new $__variant_{}_{index} (i32.const {index}) {payload})",
            self.interface_id(ty).unwrap().0
        )
    }
    pub(super) fn variant_lift(&self, ty: &Type, ptr: &str) -> String {
        let cases = self
            .variant_payloads(ty)
            .iter()
            .enumerate()
            .map(|(i, t)| {
                self.variant_construct(
                    ty,
                    i,
                    &t.as_ref()
                        .map(|t| self.lift(t, &pointer_at(ptr, self.variant_offset(ty))))
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        select(
            &self.variant_tag(ty, ptr),
            Some(&core_type(ty, self.module)),
            &cases,
        )
    }
    pub(super) fn variant_lower(&self, ty: &Type, value: &str, ptr: &str) -> String {
        let id = self.interface_id(ty).unwrap();
        let tag = format!("(struct.get {} 0 {value})", self.module.types[id.0].name);
        let store = match self.variant_tag_width(ty) {
            1 => "i32.store8",
            2 => "i32.store16",
            _ => "i32.store",
        };
        let cases = self
            .variant_payloads(ty)
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let cast = format!("(ref.cast (ref $__variant_{}_{i}) {value})", id.0);
                let payload = t
                    .as_ref()
                    .map(|t| {
                        self.lower(
                            t,
                            &format!("(struct.get $__variant_{}_{i} 1 {cast})", id.0),
                            &pointer_at(ptr, self.variant_offset(ty)),
                        )
                    })
                    .unwrap_or_default();
                format!("(block (drop {cast}) ({store} {ptr} (i32.const {i})) {payload})")
            })
            .collect::<Vec<_>>();
        select(&tag, None, &cases)
    }
    pub(super) fn variant_release(&self, ty: &Type, ptr: &str) -> String {
        let cases = self
            .variant_payloads(ty)
            .iter()
            .map(|t| {
                t.as_ref()
                    .map(|t| self.release(t, &pointer_at(ptr, self.variant_offset(ty))))
                    .unwrap_or("(nop)".into())
            })
            .collect::<Vec<_>>();
        select(&self.variant_tag(ty, ptr), None, &cases)
    }
    pub(super) fn variant_flat_load(&self, ty: &Type, ptr: &str) -> Vec<(String, NumType)> {
        let tag = self.variant_tag(ty, ptr);
        let payloads = self
            .variant_payloads(ty)
            .iter()
            .map(|t| {
                t.as_ref()
                    .map(|t| self.flat_load(t, &pointer_at(ptr, self.variant_offset(ty))))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let mut result = vec![(tag.clone(), NumType::I32)];
        for (i, n) in self.variant_flat(ty).into_iter().skip(1).enumerate() {
            let cases = payloads
                .iter()
                .map(|p| {
                    p.get(i)
                        .map(|(s, t)| coerce(s, *t, n))
                        .unwrap_or_else(|| format!("({}.const 0)", n.wat()))
                })
                .collect::<Vec<_>>();
            result.push((select(&tag, Some(n.wat()), &cases), n));
        }
        result
    }
    pub(super) fn variant_flat_lift(&self, ty: &Type, values: &[(String, NumType)]) -> String {
        let cases = self
            .variant_payloads(ty)
            .iter()
            .enumerate()
            .map(|(i, t)| {
                self.variant_construct(
                    ty,
                    i,
                    &t.as_ref()
                        .map(|t| self.flat_lift(t, &values[1..]))
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        select(&values[0].0, Some(&core_type(ty, self.module)), &cases)
    }
    pub(super) fn variant_codec(&self, ty: &Type, names: &[String]) -> (String, String) {
        let names_json = serde_json::to_string(names).unwrap();
        let offset = self.variant_offset(ty);
        let bits = self.variant_tag_width(ty) * 8;
        let cases = self.variant_payloads(ty);
        let read = cases
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "case {i}:return {{tag:names[n]{}}};",
                    t.as_ref()
                        .map(|t| format!(",val:r_{}(p+{offset})", type_key(t)))
                        .unwrap_or_default()
                )
            })
            .collect::<String>();
        let write=cases.iter().enumerate().map(|(i,t)|format!("case {i}:{}break;",t.as_ref().map(|t|format!("w_{}(p+{offset},v.val);",type_key(t))).unwrap_or_else(||"if(v.val!==undefined)throw new TypeError('unexpected variant payload');".into()))).collect::<String>();
        (
            format!(
                "const names={names_json},n=c.view().getUint{bits}(p,true);switch(n){{{read}default:throw new TypeError('invalid variant tag');}}"
            ),
            format!(
                "if(v===null||typeof v!=='object')throw new TypeError('expected variant');const n={names_json}.indexOf(v.tag);if(n<0)throw new TypeError('invalid variant tag');c.view().setUint{bits}(p,n,true);switch(n){{{write}}}"
            ),
        )
    }
}
