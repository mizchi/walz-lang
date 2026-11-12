//! Canonical list storage is an owned ptr/length pair, separate from GC arrays.
use super::{Adapter, MEMORY_LIMIT, abi::pointer_at};
use crate::ir::*;

impl Adapter<'_> {
    pub(super) fn list_helpers(&self, id: TypeId) -> String {
        let n = id.0;
        let name = &self.module.types[n].name;
        let ValueType::List(payload) = self.value(id) else {
            unreachable!()
        };
        let layout = self.layout(payload);
        let stride = layout.size;
        let alignment = layout.align;
        let storage = format!("$__list_storage_{n}");
        let at = "(i32.add (local.get $q) (i32.mul (local.get $i) (i32.const STRIDE)))"
            .replace("STRIDE", &stride.to_string());
        let limit = MEMORY_LIMIT / stride;
        let guard=format!("(if (i32.or (i32.gt_u (local.get $len) (i32.const {limit})) (i32.ne (i32.and (local.get $q) (i32.const {})) (i32.const 0))) (then (unreachable)))
          (if (i64.gt_u (i64.add (i64.extend_i32_u (local.get $q)) (i64.mul (i64.extend_i32_u (local.get $len)) (i64.const {stride}))) (i64.mul (i64.extend_i32_u (memory.size)) (i64.const 65536))) (then (unreachable)))",alignment-1);
        let (first, next) = if *payload == Type::Unit {
            (
                format!("(array.new_default {storage} (local.get $len))"),
                self.lift(payload, &at),
            )
        } else {
            (
                format!(
                    "(array.new {storage} {} (local.get $len))",
                    self.lift(payload, "(local.get $q)")
                ),
                format!(
                    "(array.set {storage} (local.get $a) (local.get $i) {})",
                    self.lift(payload, &at)
                ),
            )
        };
        let start = if *payload == Type::Unit { 0 } else { 1 };
        let value = if *payload == Type::Unit {
            String::new()
        } else {
            format!("(array.get {storage} (local.get $a) (local.get $i))")
        };
        let lower = self.lower(payload, &value, &at);
        let release = self.release(payload, &at);
        let descriptor_q = pointer_at("(local.get $p)", 4);
        // Nondefaultable references use the first lifted element to initialize
        // their GC array. The first element is lifted exactly once.
        format!("(func $__lift_list_{n} (param $q i32) (param $len i32) (result (ref {name}))
          (local $a (ref {storage})) (local $i i32) {guard}
          (local.set $a (if (result (ref {storage})) (local.get $len)
            (then {first}) (else (array.new_fixed {storage} 0))))
          (local.set $i (i32.const {start}))
          (loop $copy (if (i32.lt_u (local.get $i) (local.get $len)) (then {next}
            (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy))))
          (if (local.get $len) (then (call $__free (local.get $q))))
          (struct.new {name} (local.get $a)))
        (func $__lower_list_{n} (param $v (ref {name})) (param $p i32)
          (local $a (ref {storage})) (local $q i32) (local $len i32) (local $i i32)
          (local.set $a (struct.get {name} 0 (local.get $v)))
          (local.set $len (array.len (local.get $a)))
          (if (i32.gt_u (local.get $len) (i32.const {limit})) (then (unreachable)))
          (if (local.get $len) (then (local.set $q (call $__alloc (i32.mul (local.get $len) (i32.const {stride}))))))
          (i32.store (local.get $p) (local.get $q)) (i32.store {descriptor_q} (local.get $len))
          (loop $copy (if (i32.lt_u (local.get $i) (local.get $len)) (then {lower}
            (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $copy)))))
        (func $__release_list_{n} (param $p i32) (local $q i32) (local $len i32) (local $i i32)
          (local.set $q (i32.load (local.get $p))) (local.set $len (i32.load {descriptor_q}))
          (loop $release (if (i32.lt_u (local.get $i) (local.get $len)) (then {release}
            (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $release))))
          (if (local.get $len) (then (call $__free (local.get $q)))))\n")
    }
}
