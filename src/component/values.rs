//! Copy values between private GC storage and owned canonical buffers.
use super::abi::{align, coerce, core_type, load, pointer_at, type_key};
use super::{Adapter, MEMORY_LIMIT};
use crate::ir::*;

impl Adapter<'_> {
    pub(super) fn value_helpers(&self) -> String {
        let mut out = String::new();
        for ty in &self.types {
            let k = type_key(ty);
            let result = if *ty == Type::Unit {
                String::new()
            } else {
                format!(" (result {})", core_type(ty, self.module))
            };
            let param = if *ty == Type::Unit {
                String::new()
            } else {
                format!(" (param $v {})", core_type(ty, self.module))
            };
            out.push_str(&format!(
                "(func $__release_{k} (param $p i32) {})\n",
                self.released_memory(ty, "(local.get $p)")
            ));
            if let Type::Value(id) = ty
                && matches!(self.value(*id), ValueType::Bytes | ValueType::Text)
            {
                out.push_str(&self.buffer_helpers(*id));
            }
            out.push_str(&format!("(func $__lift_{k} (param $p i32){result} {})\n(func $__lower_{k}{param} (param $p i32) {})\n",self.lifted_memory(ty,"(local.get $p)"),self.lowered_memory(ty,"(local.get $v)","(local.get $p)")));
        }
        out
    }
    pub(super) fn lift(&self, ty: &Type, ptr: &str) -> String {
        format!("(call $__lift_{} {ptr})", type_key(ty))
    }
    pub(super) fn lower(&self, ty: &Type, value: &str, ptr: &str) -> String {
        format!(
            "(call $__lower_{} {}{ptr})",
            type_key(ty),
            if *ty == Type::Unit {
                String::new()
            } else {
                format!("{value} ")
            }
        )
    }
    fn construct(&self, id: TypeId, case: usize, payload: &[(Type, String)]) -> String {
        let checks = payload
            .iter()
            .filter(|(t, _)| *t == Type::Unit)
            .map(|(_, s)| format!(" {s}"))
            .collect::<String>();
        let args = payload
            .iter()
            .filter(|(t, _)| *t != Type::Unit)
            .map(|(_, s)| format!(" {s}"))
            .collect::<String>();
        format!(
            "(block (result {}){checks} (struct.new $__variant_{}_{case} (i32.const {case}){args}))",
            core_type(&Type::Value(id), self.module),
            id.0
        )
    }
    pub(super) fn release(&self, ty: &Type, ptr: &str) -> String {
        format!("(call $__release_{} {ptr})", type_key(ty))
    }
    fn released_memory(&self, ty: &Type, ptr: &str) -> String {
        let Type::Value(id) = ty else {
            return "(nop)".into();
        };
        match self.value(*id) {
            ValueType::Bytes | ValueType::Text => format!(
                "(if (i32.load {}) (then (call $__free (i32.load {ptr}))))",
                pointer_at(ptr, 4)
            ),
            ValueType::Pair { first, second } => format!(
                "{} {}",
                self.release(first, ptr),
                self.release(
                    second,
                    &pointer_at(
                        ptr,
                        align(self.layout(first).size, self.layout(second).align)
                    )
                )
            ),
            ValueType::Option(_) | ValueType::Result { .. } => {
                let cases = self
                    .cases(*id)
                    .iter()
                    .map(|case| {
                        case.as_ref()
                            .map(|t| {
                                self.release(t, &pointer_at(ptr, align(1, self.layout(ty).align)))
                            })
                            .unwrap_or_else(|| "(nop)".into())
                    })
                    .collect::<Vec<_>>();
                format!(
                    "(if (i32.eqz (i32.load8_u {ptr})) (then {}) (else {}))",
                    cases[0], cases[1]
                )
            }
            _ => "(nop)".into(),
        }
    }
    fn buffer_helpers(&self, id: TypeId) -> String {
        let n = id.0;
        let name = &self.module.types[n].name;
        let validate = if matches!(self.value(id), ValueType::Text) {
            "(if (i32.eqz (call $__utf8 (local.get $a))) (then (unreachable)))"
        } else {
            ""
        };
        format!(
            r#"
          (func $__lift_buffer_{n} (param $p i32) (param $len i32) (result (ref {name}))
            (local $a (ref $__byte_storage)) (local $i i32)
            (if (i32.gt_u (local.get $len) (i32.const {MEMORY_LIMIT})) (then (unreachable)))
            (if (i64.gt_u (i64.add (i64.extend_i32_u (local.get $p)) (i64.extend_i32_u (local.get $len))) (i64.mul (i64.extend_i32_u (memory.size)) (i64.const 65536))) (then (unreachable)))
            (local.set $a (array.new_default $__byte_storage (local.get $len)))
            (loop $copy (if (i32.lt_u (local.get $i) (local.get $len)) (then
              (array.set $__byte_storage (local.get $a) (local.get $i) (i32.load8_u (i32.add (local.get $p) (local.get $i))))
              (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
            {validate} (if (local.get $len) (then (call $__free (local.get $p)))) (struct.new {name} (local.get $a)))
          (func $__lower_flat_buffer_{n} (param $v (ref {name})) (result i32)
            (local $a (ref $__byte_storage)) (local $q i32) (local $len i32) (local $i i32)
            (local.set $a (struct.get {name} 0 (local.get $v))) (local.set $len (array.len (local.get $a)))
            (if (i32.gt_u (local.get $len) (i32.const {MEMORY_LIMIT})) (then (unreachable)))
            (if (local.get $len) (then (local.set $q (call $__alloc (local.get $len)))))
            (loop $copy (if (i32.lt_u (local.get $i) (local.get $len)) (then
              (i32.store8 (i32.add (local.get $q) (local.get $i)) (array.get_u $__byte_storage (local.get $a) (local.get $i)))
              (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
            (local.get $q))
          (func $__lower_buffer_{n} (param $v (ref {name})) (param $p i32)
            (local $q i32) (local $len i32)
            (local.set $len (array.len (struct.get {name} 0 (local.get $v))))
            (local.set $q (call $__lower_flat_buffer_{n} (local.get $v)))
            (i32.store (local.get $p) (local.get $q)) (i32.store offset=4 (local.get $p) (local.get $len)))
        "#
        )
    }
    fn lifted_memory(&self, ty: &Type, ptr: &str) -> String {
        match ty {
            Type::Num(n) => load(*n, ptr),
            Type::Unit => {
                format!("(if (i32.ne (i32.load8_u {ptr}) (i32.const 0)) (then (unreachable)))")
            }
            Type::Value(id) => match self.value(*id) {
                ValueType::Bytes | ValueType::Text => format!(
                    "(call $__lift_buffer_{} (i32.load {ptr}) (i32.load {}))",
                    id.0,
                    pointer_at(ptr, 4)
                ),
                ValueType::Future(_) | ValueType::Stream(_) => {
                    self.remote(*id, &load(NumType::I32, ptr))
                }
                ValueType::Pair { first, second } => {
                    let offset = align(self.layout(first).size, self.layout(second).align);
                    self.construct(
                        *id,
                        0,
                        &[
                            (first.clone(), self.lift(first, ptr)),
                            (second.clone(), self.lift(second, &pointer_at(ptr, offset))),
                        ],
                    )
                }
                _ => {
                    let tag = format!("(i32.load8_u {ptr})");
                    let offset = align(1, self.layout(ty).align);
                    let bodies = self
                        .cases(*id)
                        .iter()
                        .enumerate()
                        .map(|(case, t)| {
                            self.construct(
                                *id,
                                case,
                                &t.as_ref()
                                    .map(|t| {
                                        vec![(t.clone(), self.lift(t, &pointer_at(ptr, offset)))]
                                    })
                                    .unwrap_or_default(),
                            )
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "(if (result {}) (i32.eq {tag} (i32.const 0)) (then {}) (else (if (result {}) (i32.eq {tag} (i32.const 1)) (then {}) (else (unreachable)))))",
                        core_type(ty, self.module),
                        bodies[0],
                        core_type(ty, self.module),
                        bodies[1]
                    )
                }
            },
            _ => unreachable!(),
        }
    }
    fn lowered_memory(&self, ty: &Type, value: &str, ptr: &str) -> String {
        match ty {
            Type::Num(n) => format!("({}.store {ptr} {value})", n.wat()),
            Type::Unit => format!("(i32.store8 {ptr} (i32.const 0))"),
            Type::Value(id) => match self.value(*id) {
                ValueType::Bytes | ValueType::Text => {
                    format!("(call $__lower_buffer_{} {value} {ptr})", id.0)
                }
                ValueType::Future(_) | ValueType::Stream(_) => {
                    format!("(i32.store {ptr} (call $__lower_handle_{} {value}))", id.0)
                }
                ValueType::Pair { first, second } => {
                    let offset = align(self.layout(first).size, self.layout(second).align);
                    let mut field = 1;
                    let mut writes = String::new();
                    for (t, p) in [(first, ptr.to_owned()), (second, pointer_at(ptr, offset))] {
                        let read = if *t == Type::Unit {
                            String::new()
                        } else {
                            let s = format!(
                                "(struct.get $__variant_{}_0 {field} (ref.cast (ref $__variant_{}_0) {value}))",
                                id.0, id.0
                            );
                            field += 1;
                            s
                        };
                        writes.push_str(&self.lower(t, &read, &p));
                    }
                    format!(
                        "(block (drop (ref.cast (ref $__variant_{}_0) {value})) {writes})",
                        id.0
                    )
                }
                _ => {
                    let tag = format!("(struct.get {} 0 {value})", self.module.types[id.0].name);
                    let offset = align(1, self.layout(ty).align);
                    let cases = self
                        .cases(*id)
                        .iter()
                        .enumerate()
                        .map(|(case, t)| {
                            let cast =
                                format!("(ref.cast (ref $__variant_{}_{case}) {value})", id.0);
                            let write = t
                                .as_ref()
                                .map(|t| {
                                    self.lower(
                                        t,
                                        &format!(
                                            "(struct.get $__variant_{}_{case} 1 {cast})",
                                            id.0
                                        ),
                                        &pointer_at(ptr, offset),
                                    )
                                })
                                .unwrap_or_default();
                            format!("(block (drop {cast}) {write})")
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "(block (i32.store8 {ptr} {tag}) (if (i32.eq {tag} (i32.const 0)) (then {}) (else (if (i32.eq {tag} (i32.const 1)) (then {}) (else (unreachable))))))",
                        cases[0], cases[1]
                    )
                }
            },
            _ => unreachable!(),
        }
    }
    pub(super) fn flat_lift(&self, ty: &Type, values: &[(String, NumType)]) -> String {
        match ty {
            Type::Num(n) => coerce(&values[0].0, values[0].1, *n),
            Type::Unit => format!(
                "(if (i32.ne {} (i32.const 0)) (then (unreachable)))",
                values[0].0
            ),
            Type::Value(id) => match self.value(*id) {
                ValueType::Bytes | ValueType::Text => format!(
                    "(call $__lift_buffer_{} {} {})",
                    id.0, values[0].0, values[1].0
                ),
                ValueType::Future(_) | ValueType::Stream(_) => self.remote(*id, &values[0].0),
                ValueType::Pair { first, second } => {
                    let n = self.flat(first).len();
                    self.construct(
                        *id,
                        0,
                        &[
                            (first.clone(), self.flat_lift(first, &values[..n])),
                            (second.clone(), self.flat_lift(second, &values[n..])),
                        ],
                    )
                }
                _ => {
                    let bodies = self
                        .cases(*id)
                        .iter()
                        .enumerate()
                        .map(|(case, t)| {
                            self.construct(
                                *id,
                                case,
                                &t.as_ref()
                                    .map(|t| vec![(t.clone(), self.flat_lift(t, &values[1..]))])
                                    .unwrap_or_default(),
                            )
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "(if (result {}) (i32.eq {} (i32.const 0)) (then {}) (else (if (result {}) (i32.eq {} (i32.const 1)) (then {}) (else (unreachable)))))",
                        core_type(ty, self.module),
                        values[0].0,
                        bodies[0],
                        core_type(ty, self.module),
                        values[0].0,
                        bodies[1]
                    )
                }
            },
            _ => unreachable!(),
        }
    }
}
