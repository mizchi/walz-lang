//! Standard Canonical ABI producers. Input copies live in this component's
//! allocator until writes acknowledge them; no host delivery factory is used.
use super::{Adapter, abi::component_type};
use crate::{Error, ir::*};

pub(super) fn body(a: &Adapter<'_>) -> Result<String, Error> {
    if a.endpoints.is_empty() {
        return Ok(String::new());
    }
    let mut canonical = String::new();
    let mut imports = String::new();
    let mut exports = String::new();
    let mut functions = String::new();
    let mut lifted = String::new();
    let helpers = format!("(module {})", a.value_helpers());
    let syntax = crate::sexpr::parse(&helpers)?;
    let releases = syntax
        .list()
        .unwrap()
        .iter()
        .skip(1)
        .filter(|n| {
            n.head() == Some("func")
                && n.list()
                    .and_then(|p| p.get(1))
                    .and_then(crate::sexpr::Sexp::atom)
                    .is_some_and(|n| n.starts_with("$__release_"))
        })
        .map(|n| helpers[n.span.start..n.span.end].to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    for id in &a.endpoints {
        let n = id.0;
        let (kind, payload) = match a.value(*id) {
            ValueType::Future(t) => ("future", t),
            ValueType::Stream(t) => ("stream", t),
            _ => unreachable!(),
        };
        let list = format!("$native_list{n}");
        canonical += &format!(
            "(type {list} (list {}))\n(core func $new{n} (canon {kind}.new $t{n}))\n(core func $write{n} (canon {kind}.write $t{n} (memory (core memory $native_mem \"memory\"))))\n(core func $dropw{n} (canon {kind}.drop-writable $t{n}))\n(core func $return{n} (canon task.return (result $t{n}) (memory (core memory $native_mem \"memory\"))))\n",
            component_type(payload)
        );
        imports += &format!(
            "(import \"\" \"new{n}\" (func $new{n} (result i64)))\n(import \"\" \"write{n}\" (func $write{n} (param i32 i32{}) (result i32)))\n(import \"\" \"dropw{n}\" (func $dropw{n} (param i32)))\n(import \"\" \"return{n}\" (func $return{n} (param i32)))\n",
            if kind == "stream" { " i32" } else { "" }
        );
        exports += &format!(
            "(export \"new{n}\" (func $new{n})) (export \"write{n}\" (func $write{n})) (export \"dropw{n}\" (func $dropw{n})) (export \"return{n}\" (func $return{n}))\n"
        );
        if kind == "stream" {
            canonical += &format!(
                "(core func $read{n} (canon stream.read $t{n} (memory (core memory $native_mem \"memory\")) (realloc (core func $native_mem \"realloc\"))))\n(core func $dropr{n} (canon stream.drop-readable $t{n}))\n"
            );
            imports += &format!(
                "(import \"\" \"read{n}\" (func $read{n} (param i32 i32 i32) (result i32)))\n(import \"\" \"dropr{n}\" (func $dropr{n} (param i32)))\n"
            );
            exports += &format!(
                "(export \"read{n}\" (func $read{n})) (export \"dropr{n}\" (func $dropr{n}))\n"
            );
        }
        for prepend in [false, true]
            .into_iter()
            .filter(|p| !*p || kind == "stream")
        {
            let name = if prepend {
                format!("prepend-t{n}")
            } else {
                format!("ready-t{n}")
            };
            let stride = a.layout(payload).size;
            let write = if kind == "future" {
                format!("(drop (call $write{n} (local.get $w) (local.get $p)))")
            } else {
                format!("(block $closed (loop $send
                (br_if $closed (i32.ge_u (local.get $i) (local.get $len)))
                (local.set $code (call $write{n} (local.get $w) (i32.add (local.get $p) (i32.mul (local.get $i) (i32.const {stride}))) (i32.sub (local.get $len) (local.get $i))))
                (local.set $i (i32.add (local.get $i) (i32.shr_u (local.get $code) (i32.const 4))))
                (br_if $closed (i32.eq (i32.and (local.get $code) (i32.const 15)) (i32.const 1))) (br $send)))")
            };
            let release = a.release(
                payload,
                &format!("(i32.add (local.get $p) (i32.mul (local.get $i) (i32.const {stride})))"),
            );
            let forward = if prepend {
                format!("(if (i32.ne (i32.and (local.get $code) (i32.const 15)) (i32.const 1)) (then
                  (local.set $buffer (call $__alloc (i32.const {stride})))
                  (block $eof (loop $forward
                    (local.set $code (call $read{n} (local.get $tail) (local.get $buffer) (i32.const 1)))
                    (if (i32.shr_u (local.get $code) (i32.const 4)) (then
                      (local.set $written (call $write{n} (local.get $w) (local.get $buffer) (i32.const 1))) {}
                      (br_if $eof (i32.eq (i32.and (local.get $written) (i32.const 15)) (i32.const 1)))))
                    (br_if $eof (i32.eq (i32.and (local.get $code) (i32.const 15)) (i32.const 1))) (br $forward)))
                  (call $__free (local.get $buffer)))) (call $dropr{n} (local.get $tail))",a.release(payload,"(local.get $buffer)"))
            } else {
                String::new()
            };
            functions+=&format!("(func (export \"{name}\") (param $p i32) (param $len i32) {} (local $ends i64) (local $w i32) (local $i i32) (local $code i32) (local $written i32) (local $buffer i32)
              {} (local.set $ends (call $new{n})) (local.set $w (i32.wrap_i64 (i64.shr_u (local.get $ends) (i64.const 32))))
              (call $return{n} (i32.wrap_i64 (local.get $ends))) {write}
              (local.set $i (i32.const 0)) (loop $release (if (i32.lt_u (local.get $i) (local.get $len)) (then {release} (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $release))))
              (if (local.get $len) (then (call $__free (local.get $p)))) {forward} (call $dropw{n} (local.get $w)))\n",if prepend{"(param $tail i32)"}else{""},if kind=="future"{"(if (i32.ne (local.get $len) (i32.const 1)) (then (unreachable)))"}else{""});
            lifted += &format!(
                "(func (export \"{name}\") async (param \"items\" {list}) {} (result $t{n}) (canon lift (core func $native_producers \"{name}\") async (memory (core memory $native_mem \"memory\")) (realloc (core func $native_mem \"realloc\"))))\n",
                if prepend {
                    format!("(param \"tail\" $t{n})")
                } else {
                    String::new()
                }
            );
        }
    }
    let text=format!("(core module $native_mem {})\n(core instance $native_mem (instantiate $native_mem))\n{canonical}
      (core module $native_producers (import \"\" \"memory\" (memory 1 256)) (import \"\" \"alloc\" (func $__alloc (param i32) (result i32))) (import \"\" \"free\" (func $__free (param i32))) {imports} {releases} {functions})
      (core instance $native_producers (instantiate $native_producers (with \"\" (instance (export \"memory\" (memory $native_mem \"memory\")) (export \"alloc\" (func $native_mem \"alloc\")) (export \"free\" (func $native_mem \"free\")) {exports}))))\n{lifted}",include_str!("../component-memory.wat"));
    Ok(text)
}
