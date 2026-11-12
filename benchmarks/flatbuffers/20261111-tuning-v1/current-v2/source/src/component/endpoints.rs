//! Readable Future/Stream handles, transfer, reads and close.
use super::abi::core_type;
use super::{Adapter, MEMORY_LIMIT};
use crate::ir::*;

impl Adapter<'_> {
    pub(super) fn remote(&self, id: TypeId, handle: &str) -> String {
        format!("(call $__remote_new_{} {handle})", id.0)
    }
    pub(crate) fn remote_types(&self, id: TypeId) -> String {
        if !self.endpoints.contains(&id) {
            return String::new();
        }
        format!(
            "(type $__remote_{} (sub final {} (struct (field i32) (field i32) (field (ref $__task_entry)))))\n",
            id.0, self.module.types[id.0].name
        )
    }
    pub(crate) fn delivery(&self, op: DeliveryOp, id: TypeId, operand: &str) -> Option<String> {
        if !self.endpoints.contains(&id) {
            return None;
        }
        let n = id.0;
        match op {
            DeliveryOp::FutureAwait | DeliveryOp::StreamNext => {
                Some(format!("(call $__receive{n} {operand})"))
            }
            DeliveryOp::FutureClose | DeliveryOp::StreamClose => {
                Some(format!("(call $__close{n} {operand})"))
            }
            _ => None,
        }
    }
    pub(super) fn endpoint_helpers(&self, id: TypeId) -> String {
        let n = id.0;
        let future = matches!(self.value(id), ValueType::Future(_));
        let payload = match self.value(id) {
            ValueType::Future(t) | ValueType::Stream(t) => t,
            _ => unreachable!(),
        };
        let layout = self.layout(payload);
        let root = core_type(&Type::Value(id), self.module);
        let remote_tag = if future { 1 } else { 2 };
        let tag = format!(
            "(struct.get {} 0 (local.get $v))",
            self.module.types[n].name
        );
        let handle =
            format!("(struct.get $__remote_{n} 1 (ref.cast (ref $__remote_{n}) (local.get $v)))");
        let release = format!(
            "(call $__task_release (struct.get $__remote_{n} 2 (ref.cast (ref $__remote_{n}) (local.get $v))))"
        );
        let release_end = format!(
            "(call $__task_release (struct.get $__remote_{n} 2 (ref.cast (ref $__remote_{n}) (local.get $end))))"
        );
        let register = format!("(func $__remote_new_{n} (param $h i32) (result {root})
          (struct.new $__remote_{n} (i32.const {remote_tag}) (local.get $h) (call $__task_register (i32.const {n}) (local.get $h))))\n");
        let close = format!(
            "(func $__close{n} (param $v {root}) (if (i32.eq {tag} (i32.const {remote_tag})) (then (call $__drop{n} {handle}) {release})))\n"
        );
        let lower = if future {
            let value = if *payload == Type::Unit {
                String::new()
            } else {
                format!(
                    "(struct.get $__variant_{n}_0 1 (ref.cast (ref $__variant_{n}_0) (local.get $v)))"
                )
            };
            format!("(func $__lower_handle_{n} (param $v {root}) (result i32) (local $p i32) (local $h i32)
              (if (i32.eq {tag} (i32.const 1)) (then {release} (return {handle})))
              (drop (ref.cast (ref $__variant_{n}_0) (local.get $v)))
              (local.set $p (call $__alloc (i32.const {}))) {}
              (local.set $h (call $__ready{n} (local.get $p) (i32.const 1)))
              {} (call $__free (local.get $p)) (local.get $h))\n",layout.size,self.lower(payload,&value,"(local.get $p)"),self.release(payload,"(local.get $p)"))
        } else {
            let tail_field = if *payload == Type::Unit { 1 } else { 2 };
            let cursor_tag = format!(
                "(struct.get {} 0 (local.get $cursor))",
                self.module.types[n].name
            );
            let tail = format!(
                "(struct.get $__variant_{n}_1 {tail_field} (ref.cast (ref $__variant_{n}_1) (local.get $cursor)))"
            );
            let item = if *payload == Type::Unit {
                String::new()
            } else {
                format!(
                    "(struct.get $__variant_{n}_1 1 (ref.cast (ref $__variant_{n}_1) (local.get $cursor)))"
                )
            };
            format!("(func $__lower_handle_{n} (param $v {root}) (result i32) (local $cursor {root}) (local $end {root}) (local $count i32) (local $i i32) (local $p i32) (local $h i32)
              (if (i32.eq {tag} (i32.const 2)) (then {release} (return {handle})))
              (local.set $cursor (local.get $v))
              (loop $count (if (i32.eq {cursor_tag} (i32.const 1)) (then
                (local.set $count (i32.add (local.get $count) (i32.const 1)))
                (if (i32.gt_u (local.get $count) (i32.const {})) (then (unreachable)))
                (local.set $cursor {tail}) (br $count))))
              (local.set $end (local.get $cursor)) (local.set $cursor (local.get $v))
              (local.set $p (call $__alloc (i32.mul (local.get $count) (i32.const {}))))
              (loop $fill (if (i32.lt_u (local.get $i) (local.get $count)) (then
                {} (local.set $cursor {tail}) (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $fill))))
              (local.set $h (if (result i32) (i32.eq (struct.get {} 0 (local.get $end)) (i32.const 2))
                (then {release_end} (call $__prepend{n} (local.get $p) (local.get $count) (struct.get $__remote_{n} 1 (ref.cast (ref $__remote_{n}) (local.get $end)))))
                (else (call $__ready{n} (local.get $p) (local.get $count)))))
              (local.set $i (i32.const 0)) (loop $release (if (i32.lt_u (local.get $i) (local.get $count)) (then {} (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $release)))) (call $__free (local.get $p)) (local.get $h))\n",MEMORY_LIMIT/layout.size,layout.size,self.lower(payload,&item,&format!("(i32.add (local.get $p) (i32.mul (local.get $i) (i32.const {})))",layout.size)),self.module.types[n].name,self.release(payload,&format!("(i32.add (local.get $p) (i32.mul (local.get $i) (i32.const {})))",layout.size)))
        };
        let receive = if future {
            let result = if *payload == Type::Unit {
                String::new()
            } else {
                format!(" (result {})", core_type(payload, self.module))
            };
            let local = if *payload == Type::Unit {
                String::new()
            } else {
                format!(" (local $r {})", core_type(payload, self.module))
            };
            let read = self.lift(payload, "(local.get $p)");
            let save = if *payload == Type::Unit {
                String::new()
            } else {
                format!("(local.set $r {read})")
            };
            let ready = if *payload == Type::Unit {
                format!("(drop (ref.cast (ref $__variant_{n}_0) (local.get $v)))")
            } else {
                format!(
                    "(struct.get $__variant_{n}_0 1 (ref.cast (ref $__variant_{n}_0) (local.get $v)))"
                )
            };
            format!("(func $__receive{n} (param $v {root}){result} (local $p i32) (local $code i32){local}
              (call $__checkpoint)
              (if{result} (i32.eq {tag} (i32.const 0)) (then {ready}) (else
                (local.set $p (call $__alloc (i32.const {})))
                (local.set $code (call $__read{n} {handle} (local.get $p))) (call $__checkpoint)
                (if (i32.ne (local.get $code) (i32.const 0)) (then (unreachable)))
                {save} (call $__drop{n} {handle}) {release} (call $__free (local.get $p)) {})))\n",layout.size,if *payload==Type::Unit {String::new()}else{"(local.get $r)".into()})
        } else {
            let option = self.module.value_id(&ValueType::Option(payload.clone()));
            let pair = self.module.value_id(&ValueType::Pair {
                first: Type::Value(id),
                second: Type::Value(option),
            });
            let result = core_type(&Type::Value(pair), self.module);
            let tail_field = if *payload == Type::Unit { 1 } else { 2 };
            let item = if *payload == Type::Unit {
                String::new()
            } else {
                format!(
                    " (struct.get $__variant_{n}_1 1 (ref.cast (ref $__variant_{n}_1) (local.get $v)))"
                )
            };
            let remote_item = if *payload == Type::Unit {
                String::new()
            } else {
                format!(" {}", self.lift(payload, "(local.get $p)"))
            };
            format!("(func $__receive{n} (param $v {root}) (result {result}) (local $p i32) (local $code i32) (local $next {root}) (local $r {result})
              (call $__checkpoint)
              (if (i32.eq {tag} (i32.const 0)) (then (return (struct.new $__variant_{}_0 (i32.const 0) (local.get $v) (struct.new $__variant_{}_0 (i32.const 0))))))
              (if (i32.eq {tag} (i32.const 1)) (then (return (struct.new $__variant_{}_0 (i32.const 0) (struct.get $__variant_{n}_1 {tail_field} (ref.cast (ref $__variant_{n}_1) (local.get $v))) (struct.new $__variant_{}_1 (i32.const 1){item})))))
              (local.set $p (call $__alloc (i32.const {}))) (local.set $code (call $__read{n} {handle} (local.get $p) (i32.const 1)))
              (call $__checkpoint)
              (local.set $next (if (result {root}) (i32.eq (i32.and (local.get $code) (i32.const 15)) (i32.const 1))
                (then (call $__drop{n} {handle}) {release} (struct.new $__variant_{n}_0 (i32.const 0)))
                (else (if (i32.ne (i32.and (local.get $code) (i32.const 15)) (i32.const 0)) (then (unreachable))) (local.get $v))))
              (local.set $r (if (result {result}) (i32.eq (i32.shr_u (local.get $code) (i32.const 4)) (i32.const 1))
                (then (struct.new $__variant_{}_0 (i32.const 0) (local.get $next) (struct.new $__variant_{}_1 (i32.const 1){remote_item})))
                (else (if (i32.ne (local.get $code) (i32.const 1)) (then (unreachable)))
                  (struct.new $__variant_{}_0 (i32.const 0) (local.get $next) (struct.new $__variant_{}_0 (i32.const 0))))))
              (call $__free (local.get $p)) (local.get $r))\n",pair.0,option.0,pair.0,option.0,layout.size,pair.0,option.0,pair.0,option.0)
        };
        format!("{register}{close}{lower}{receive}")
    }
}
