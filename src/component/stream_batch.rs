//! The batch primitive consumes one stream and returns its remaining owner.
//! Ready-only order is modeled by WalzDelivery.readMany_preserves_order.
use super::{Adapter, MEMORY_LIMIT, abi::core_type};
use crate::ir::*;
use std::collections::BTreeSet;

pub(crate) fn helpers(module: &Module, adapter: Option<&Adapter<'_>>) -> String {
    let mut used = BTreeSet::new();
    let mut pending = module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .collect::<Vec<_>>();
    while let Some(e) = pending.pop() {
        if let ExprKind::Delivery {
            op: DeliveryOp::StreamReadBatch,
            endpoint,
            ..
        } = e.kind
        {
            used.insert(endpoint);
        }
        pending.extend(e.children());
    }
    used.into_iter()
        .map(|id| helper(module, adapter, id))
        .collect()
}
fn helper(module: &Module, adapter: Option<&Adapter<'_>>, id: TypeId) -> String {
    let n = id.0;
    let TypeKind::Value(ValueType::Stream(payload)) = &module.types[n].kind else {
        unreachable!()
    };
    let list = module.value_id(&ValueType::List(payload.clone()));
    let pair = module.value_id(&ValueType::Pair {
        first: Type::Value(id),
        second: Type::Value(list),
    });
    let root = core_type(&Type::Value(id), module);
    let result = core_type(&Type::Value(pair), module);
    let list_name = &module.types[list.0].name;
    let storage = format!("$__list_storage_{}", list.0);
    let tag = |value: &str| format!("(struct.get {} 0 {value})", module.types[n].name);
    let cursor = "(local.get $cursor)";
    let cast = format!("(ref.cast (ref $__variant_{n}_1) {cursor})");
    let tail = format!(
        "(struct.get $__variant_{n}_1 {} {cast})",
        if *payload == Type::Unit { 1 } else { 2 }
    );
    let item = format!("(struct.get $__variant_{n}_1 1 {cast})");
    let array = if *payload == Type::Unit {
        format!("(array.new_default {storage} (local.get $count))")
    } else {
        format!("(array.new {storage} {item} (local.get $count))")
    };
    let store = if *payload == Type::Unit {
        String::new()
    } else {
        format!("(array.set {storage} (local.get $a) (local.get $i) {item})")
    };
    let pack = format!(
        "(struct.new $__variant_{}_0 (i32.const 0) {cursor} (struct.new {list_name} (local.get $a)))",
        pair.0
    );
    let remote=adapter.filter(|a|a.endpoints.contains(&id)).map(|a|{
        let size=a.layout(payload).size;
        let handle=format!("(struct.get $__remote_{n} 1 (ref.cast (ref $__remote_{n}) (local.get $v)))");
        let release=format!("(call $__task_release (struct.get $__remote_{n} 2 (ref.cast (ref $__remote_{n}) (local.get $v))))");
        let at=format!("(i32.add (local.get $p) (i32.mul (local.get $i) (i32.const {size})))");
        let lift=a.lift(payload,&at);
        let array=if *payload==Type::Unit{format!("(array.new_default {storage} (local.get $count))")}else{format!("(array.new {storage} {} (local.get $count))",a.lift(payload,"(local.get $p)"))};
        let fill=if *payload==Type::Unit{"(nop)".into()}else{format!("(array.set {storage} (local.get $a) (local.get $i) {lift})")};
        let start=if *payload==Type::Unit{0}else{1};
        format!("(if (i32.eq {} (i32.const 2)) (then
          (call $__checkpoint)
          (if (i32.gt_u (local.get $max) (i32.const {})) (then (unreachable)))
          (local.set $p (call $__alloc (i32.mul (local.get $max) (i32.const {size}))))
          (local.set $code (call $__read{n} {handle} (local.get $p) (local.get $max))) (call $__checkpoint)
          (local.set $count (i32.shr_u (local.get $code) (i32.const 4)))
          (if (i32.gt_u (local.get $count) (local.get $max)) (then (unreachable)))
          (local.set $cursor (if (result {root}) (i32.eq (i32.and (local.get $code) (i32.const 15)) (i32.const 1))
            (then (call $__drop{n} {handle}) {release} (struct.new $__variant_{n}_0 (i32.const 0)))
            (else (if (i32.or (i32.eqz (local.get $count)) (i32.ne (i32.and (local.get $code) (i32.const 15)) (i32.const 0))) (then (unreachable))) (local.get $v))))
          (local.set $a (if (result (ref {storage})) (local.get $count) (then {array}) (else (array.new_fixed {storage} 0))))
          (local.set $i (i32.const {start}))
          (loop $read (if (i32.lt_u (local.get $i) (local.get $count)) (then {fill} (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $read))))
          (call $__free (local.get $p)) (return {pack})))",tag("(local.get $v)"),MEMORY_LIMIT/size)
    }).unwrap_or_default();
    format!("(func $__read_batch_{n} (param $max i32) (param $v {root}) (result {result})
      (local $cursor {root}) (local $a (ref {storage})) (local $count i32) (local $i i32) (local $p i32) (local $code i32)
      (if (i32.or (i32.eqz (local.get $max)) (i32.gt_u (local.get $max) (i32.const 65536))) (then (unreachable)))
      {remote}
      (local.set $cursor (local.get $v))
      (loop $count (if (i32.and (i32.lt_u (local.get $count) (local.get $max)) (i32.eq {} (i32.const 1))) (then
        (local.set $count (i32.add (local.get $count) (i32.const 1))) (local.set $cursor {tail}) (br $count))))
      (local.set $cursor (local.get $v))
      (local.set $a (if (result (ref {storage})) (local.get $count) (then {array}) (else (array.new_fixed {storage} 0))))
      (loop $fill (if (i32.lt_u (local.get $i) (local.get $count)) (then {store} (local.set $cursor {tail}) (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $fill))))
      {pack})\n",tag(cursor))
}
