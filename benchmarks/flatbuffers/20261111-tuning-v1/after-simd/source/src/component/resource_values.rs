//! Canonical resource handles are private to the native world adapter. The GC
//! box carries the affine state; a borrowed box never owns its host resource.
use super::Adapter;
use crate::ir::*;
impl Adapter<'_> {
    pub(super) fn resource_helpers(&self) -> String {
        if !self.native_resources {
            return String::new();
        }
        let mut out = String::new();
        for (i, t) in self.module.types.iter().enumerate() {
            let TypeKind::Resource { .. } = t.kind else {
                continue;
            };
            let name = &t.name;
            out+=&format!("(func $__resource_new_{i} (param $h i32) (result (ref {name}))
              (struct.new {name} (local.get $h) (i32.const 0) (call $__task_register (i32.const {i}) (local.get $h))))
              (func $__resource_borrow_{i} (param $h i32) (result (ref {name})) (struct.new {name} (local.get $h) (i32.const 0) (ref.null $__task_entry)))
              (func $__resource_take_{i} (param $v (ref {name})) (result i32)
                (if (struct.get {name} 1 (local.get $v)) (then (unreachable)))
                (struct.set {name} 1 (local.get $v) (i32.const 1))
                (call $__task_release (ref.as_non_null (struct.get {name} 2 (local.get $v))))
                (struct.get {name} 0 (local.get $v)))\n");
        }
        out
    }
    pub(super) fn resource_lift(&self, ty: &Type, handle: &str) -> String {
        let (id, op) = match ty {
            Type::Resource(id) => (id, "new"),
            Type::Borrow(id) => (id, "borrow"),
            _ => unreachable!(),
        };
        format!("(call $__resource_{op}_{} {handle})", id.0)
    }
}
