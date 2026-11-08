//! Ready-only host fixture for profiling the unmodified Core/JSPI guest in
//! Wasmtime. Real JSPI and text I/O are measured separately by the Node runner.
use crate::Error;
use crate::ir::*;

pub struct Profile {
    pub env: Vec<u8>,
    pub env_wat: String,
    pub driver: Vec<u8>,
    pub driver_wat: String,
}

pub fn compile(source: &str) -> Result<Profile, Error> {
    // Share the guest's declaration projection and remapped ABI indices.
    let module = super::checked_module(source)?;
    crate::wit::validate_memories(&module)?;
    let mut read = None;
    let mut write = None;
    let mut main = None;
    for (index, f) in module.functions.iter().enumerate() {
        if let FunctionKind::Import {
            module: namespace,
            name,
        } = &f.kind
        {
            if namespace != crate::bindings::TEXT_IO {
                return Err(Error::message(
                    "profile-text accepts only text-io imports".into(),
                ));
            }
            let Type::Value(id) = f.result else {
                unreachable!()
            };
            match name.as_str() {
                "read-lines" => read = Some((index, id.0)),
                "write-line" => write = Some((index, id.0)),
                _ => unreachable!(),
            }
        }
        if f.exports.iter().any(|s| s == "main") {
            let Type::Value(id) = &f.result else {
                return Err(Error::message(
                    "profile-text needs main() -> result<i32,i32>".into(),
                ));
            };
            if !f.params.is_empty()
                || !matches!(
                    &module.types[id.0].kind,
                    TypeKind::Value(ValueType::Result {
                        ok: Type::Num(NumType::I32),
                        err: Type::Num(NumType::I32)
                    })
                )
            {
                return Err(Error::message(
                    "profile-text needs main() -> result<i32,i32>".into(),
                ));
            }
            main = Some(index);
        }
    }
    let (Some((read, s)), Some((write, f)), Some(main)) = (read, write, main) else {
        return Err(Error::message(
            "profile-text needs read-lines, write-line and main".into(),
        ));
    };
    let arena = include_str!("../component-memory.wat")
        .replace("(func $alloc (export \"alloc\")", "(func $alloc")
        .replace("(call $alloc ", "(call $profile_alloc ");
    let unicode = "日本語🦀"
        .repeat(4096)
        .bytes()
        .map(|b| format!("\\{b:02x}"))
        .collect::<String>();
    let env_wat = format!(
        r#"(module
      {arena}
      (global $allocs (mut i64) (i64.const 0))
      (global $bytes (mut i64) (i64.const 0))
      (global $peak (mut i32) (i32.const 0))
      (global $rows (mut i32) (i32.const 0))
      (global $width (mut i32) (i32.const 0))
      (global $unicode (mut i32) (i32.const 0))
      (global $cursor (mut i32) (i32.const 0))
      (global $input (mut i32) (i32.const 0))
      (global $output (mut i32) (i32.const 0))
      (global $written (mut i64) (i64.const 0))
      (data $unicode "{unicode}")
      (func $profile_alloc (export "alloc") (param $size i32) (result i32) (local $p i32)
        (local.set $p (call $alloc (local.get $size)))
        (global.set $allocs (i64.add (global.get $allocs) (i64.const 1)))
        (global.set $bytes (i64.add (global.get $bytes) (i64.extend_i32_u (local.get $size))))
        (if (i32.gt_u (global.get $count) (global.get $peak)) (then (global.set $peak (global.get $count))))
        (local.get $p))
      (func (export "alloc-count") (result i64) (global.get $allocs))
      (func (export "alloc-bytes") (result i64) (global.get $bytes))
      (func (export "peak-live") (result i32) (global.get $peak))
      (func (export "memory-bytes") (result i32) (i32.mul (memory.size) (i32.const 65536)))
      (func (export "written") (result i64) (global.get $written))
      (func (export "configure") (param $n i32) (param $size i32) (param $mode i32)
        (if (i32.or (i32.gt_u (local.get $n) (i32.const 1000000)) (i32.or (i32.eqz (local.get $size)) (i32.gt_u (local.get $size) (i32.const 53248)))) (then unreachable))
        (if (local.get $mode) (then (if (i32.rem_u (local.get $size) (i32.const 13)) (then unreachable))))
        (if (global.get $count) (then unreachable))
        (global.set $rows (local.get $n)) (global.set $width (local.get $size)) (global.set $unicode (local.get $mode))
        (global.set $allocs (i64.const 0)) (global.set $bytes (i64.const 0)) (global.set $peak (i32.const 0))
        (global.set $written (i64.const 0)))
      (func (export "host{read}") (result i32)
        (if (global.get $input) (then unreachable))
        (global.set $cursor (i32.const 0)) (global.set $input (i32.const 1)) (i32.const 1))
      (func (export "host{write}") (param $p i32) (param $len i32) (result i32)
        (if (global.get $output) (then unreachable))
        (if (i32.ne (local.get $len) (global.get $width)) (then unreachable))
        (if (i32.ne (i32.load8_u (local.get $p)) (if (result i32) (global.get $unicode) (then (i32.const 230)) (else (i32.const 120)))) (then unreachable))
        (if (i32.ne (i32.load8_u (i32.sub (i32.add (local.get $p) (local.get $len)) (i32.const 1))) (if (result i32) (global.get $unicode) (then (i32.const 128)) (else (i32.const 120)))) (then unreachable))
        (global.set $written (i64.add (global.get $written) (i64.const 1)))
        (global.set $output (i32.const 1)) (i32.const 2))
      (func (export "read{s}") (param $h i32) (param $p i32) (param $n i32) (result i32) (local $q i32) (local $len i32)
        (if (i32.or (i32.ne (local.get $h) (i32.const 1)) (i32.eqz (global.get $input))) (then unreachable))
        (if (i32.ge_u (global.get $cursor) (global.get $rows)) (then (return (i32.const 1))))
        (if (i32.and (global.get $cursor) (i32.const 7)) (then
          (local.set $len (global.get $width)) (local.set $q (call $profile_alloc (local.get $len)))
          (if (global.get $unicode)
            (then (memory.init $unicode (local.get $q) (i32.const 0) (local.get $len)))
            (else (memory.fill (local.get $q) (i32.const 120) (local.get $len))))))
        (global.set $cursor (i32.add (global.get $cursor) (i32.const 1)))
        (i32.store8 (local.get $p) (i32.const 0))
        (i32.store offset=4 (local.get $p) (local.get $q)) (i32.store offset=8 (local.get $p) (local.get $len)) (i32.const 16))
      (func (export "read{f}") (param $h i32) (param $p i32) (result i32)
        (if (i32.or (i32.ne (local.get $h) (i32.const 2)) (i32.eqz (global.get $output))) (then unreachable))
        (i32.store8 (local.get $p) (i32.const 0)) (i32.const 0))
      (func (export "drop{s}") (param $h i32)
        (if (i32.or (i32.ne (local.get $h) (i32.const 1)) (i32.eqz (global.get $input))) (then unreachable)) (global.set $input (i32.const 0)))
      (func (export "drop{f}") (param $h i32)
        (if (i32.or (i32.ne (local.get $h) (i32.const 2)) (i32.eqz (global.get $output))) (then unreachable)) (global.set $output (i32.const 0)))
      (func (export "ready{s}") (param i32 i32) (result i32) unreachable)
      (func (export "prepend{s}") (param i32 i32 i32) (result i32) unreachable)
      (func (export "ready{f}") (param i32 i32) (result i32) unreachable)
      (func (export "handles") (result i32) (i32.add (global.get $input) (global.get $output)))
    )"#
    );
    let driver_wat = format!(
        r#"(module
      (import "__component" "memory" (memory 1 256))
      (import "__component" "configure" (func $configure (param i32 i32 i32)))
      (import "__component" "alloc-count" (func $allocs (result i64)))
      (import "__component" "alloc-bytes" (func $bytes (result i64)))
      (import "__component" "peak-live" (func $peak (result i32)))
      (import "__component" "memory-bytes" (func $memory (result i32)))
      (import "__component" "live" (func $live (result i32)))
      (import "__component" "handles" (func $handles (result i32)))
      (import "__component" "written" (func $written (result i64)))
      (import "guest" "__export_{main}" (func $main (result i32)))
      (import "guest" "__post_{main}" (func $post (param i32)))
      (func (export "run") (param $rows i32) (param $width i32) (param $rounds i32) (param $unicode i32)
        (result i64 i64 i64 i32 i32 i32) (local $i i32) (local $p i32) (local $expected i32)
        (if (i32.or (i32.eqz (local.get $rounds)) (i32.gt_u (local.get $rounds) (i32.const 1000))) (then unreachable))
        (local.set $expected (i32.sub (local.get $rows) (i32.div_u (i32.add (local.get $rows) (i32.const 7)) (i32.const 8))))
        (call $configure (local.get $rows) (local.get $width) (local.get $unicode))
        (loop $run
          (local.set $p (call $main))
          (if (i32.or (i32.load8_u (local.get $p)) (i32.ne (i32.load offset=4 (local.get $p)) (local.get $expected))) (then unreachable))
          (call $post (local.get $p))
          (if (i32.or (call $live) (call $handles)) (then unreachable))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (br_if $run (i32.lt_u (local.get $i) (local.get $rounds))))
        (call $written) (call $allocs) (call $bytes) (call $peak) (call $memory) (call $live))
    )"#
    );
    Ok(Profile {
        env: parse(&env_wat)?,
        driver: parse(&driver_wat)?,
        env_wat,
        driver_wat,
    })
}
fn parse(wat: &str) -> Result<Vec<u8>, Error> {
    let bytes = wat::parse_str(wat).map_err(|e| Error::message(format!("profiling WAT: {e}")))?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(|e| Error::message(format!("profiling Wasm: {e}")))?;
    Ok(bytes)
}
