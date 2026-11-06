use std::{collections::BTreeSet, process::Command};
use wlt::ir::{BufferOp, Expr, ExprKind, Module};

const SCAN: &str = r#"(module
  (%use "wlt:core" (type $string)
    (func $string-view.new) (func $string-view.slice) (func $string-view.bytes)
    (func $string-view.byte-len) (func $bytes-view.slice) (func $bytes-view.get))
  (func $scan (param $input $string) (result i32) (%effect allocates)
    (%let (($whole (call $string-view.new $input))
           ($part (call $string-view.slice $whole 1 7))
           ($tail (call $string-view.slice $part 3 4))
           ($bytes (call $bytes-view.slice (call $string-view.bytes $tail) 0 1)))
      (+ (call $string-view.byte-len $tail) (call $bytes-view.get $bytes 0))))
  (func $main (export "main") (result i32) (%effect allocates)
    (call $scan (%string.lit "x日🦀z"))))"#;

fn no_payload_copies(module: &Module, expression: &Expr, visited: &mut BTreeSet<usize>) {
    match &expression.kind {
        ExprKind::Buffer { op, .. } => assert!(
            matches!(op, BufferOp::Get | BufferOp::Len | BufferOp::Encode),
            "unexpected payload allocation or scan: {op:?}"
        ),
        ExprKind::ArrayNew { .. } | ExprKind::ArrayFixed { .. } | ExprKind::Data(_) => {
            panic!("view operation allocated payload storage")
        }
        ExprKind::Call { function, .. } if visited.insert(function.0) => {
            if let Some(body) = &module.functions[function.0].body {
                no_payload_copies(module, body, visited);
            }
        }
        _ => {}
    }
    for child in expression.children() {
        no_payload_copies(module, child, visited);
    }
}

#[test]
fn views_read_nested_relative_ranges_without_copying_payloads() {
    let module = wlt::check(SCAN).unwrap();
    let scan = module.functions.iter().find(|f| f.name == "$scan").unwrap();
    no_payload_copies(&module, scan.body.as_ref().unwrap(), &mut BTreeSet::new());
    for optimize in [false, true] {
        let compiled = wlt::compile_with_options(SCAN, wlt::CompileOptions { optimize }).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("views.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y",
                "--invoke",
                "main",
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // The first byte of U+1F980 is 240; the text view contains four bytes.
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "244");
    }
}

#[test]
fn view_layouts_effects_and_explicit_abi_materialization_are_checked() {
    for source in [
        r#"(module (%use "wlt:core" (type $bytes-view))
          (func $bad (param $v (ref $bytes-view)) (result i32)
            (struct.get $bytes-view $length $v)))"#,
        r#"(module (%use "wlt:core" (type $string-view) (type $bytes-view))
          (func $bad (param $v (ref $bytes-view)) (result (ref $string-view))
            (struct.new $string-view $v)))"#,
    ] {
        assert_eq!(
            wlt::check(source).unwrap_err().diagnostic().code,
            "E_OPAQUE_REPRESENTATION"
        );
    }
    assert_eq!(
        wlt::check(&SCAN.replace("(%effect allocates)", "(%effect pure)"))
            .unwrap_err()
            .diagnostic()
            .code,
        "E_EFFECT_CONTRACT"
    );
    let module = wlt::check(
        r#"(module
      (%use "wlt:core" (type $bytes) (type $bytes-view) (func $bytes-view.new))
      (func $view (export "view") (param $bytes $bytes) (result (ref $bytes-view))
        (call $bytes-view.new $bytes)))"#,
    )
    .unwrap();
    assert!(
        wlt::wit::emit(&module)
            .unwrap_err()
            .to_string()
            .contains("GC references")
    );
}

#[test]
fn subviews_share_storage_without_retaining_parents_and_copies_detach() {
    let compiled = wlt::compile(
        r#"(module
      (%use "wlt:core" (type $bytes) (type $bytes-view)
        (func $bytes-view.new) (func $bytes-view.slice)
        (func $bytes-view.get) (func $bytes-view.to-bytes))
      (type $Storage (array (mut i32)))
      (func $make (export "make") (result (ref $bytes-view))
        (call $bytes-view.new (%bytes.from-array $Storage (array.new $Storage 7 131072))))
      (func $slice (export "slice") (param $v (ref $bytes-view)) (result (ref $bytes-view))
        (call $bytes-view.slice $v 3 5))
      (func $get (export "get") (param $v (ref $bytes-view)) (result i32)
        (call $bytes-view.get $v 0))
      (func $copy (export "copy") (param $v (ref $bytes-view)) (result $bytes)
        (call $bytes-view.to-bytes $v))
      (func $read (export "read") (param $v $bytes) (result i32) (%bytes.get $v 0)))"#,
    )
    .unwrap();
    let module = compiled.ir.as_ref().unwrap();
    let view = module
        .types
        .iter()
        .find(|t| t.name.ends_with(".bytes-view"))
        .unwrap();
    let bytes = module
        .types
        .iter()
        .find(|t| matches!(t.kind, wlt::ir::TypeKind::Value(wlt::ir::ValueType::Bytes)))
        .unwrap();
    // Test-only observers of private packed storage; these are never shipped.
    let observers = format!(
        r#"
      (func (export "backing") (param (ref {})) (result anyref)
        (struct.get {} 0 (struct.get {} 0 (local.get 0))))
      (func (export "ownedBacking") (param (ref {})) (result anyref)
        (struct.get {} 0 (local.get 0))))"#,
        view.name, bytes.name, view.name, bytes.name, bytes.name
    );
    let wat = compiled.wat.trim_end();
    let wasm = wat::parse_str(format!("{}{observers}", &wat[..wat.len() - 1])).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gc.wasm");
    std::fs::write(&path, wasm).unwrap();
    let output = Command::new("node").args(["--expose-gc", "--input-type=module", "-e", r#"
      import assert from 'node:assert/strict';
      import {readFile} from 'node:fs/promises';
      const {instance} = await WebAssembly.instantiate(await readFile(process.argv[1]));
      const api = instance.exports;
      let {child, copy, parentRef, sourceRef} = (() => {
        const parent = api.make(), child = api.slice(parent), copy = api.copy(child);
        assert.equal(api.backing(parent), api.backing(child), 'slice copied the payload');
        assert.notEqual(api.backing(child), api.ownedBacking(copy), 'copy retained the source');
        return {child, copy, parentRef: new WeakRef(parent), sourceRef: new WeakRef(api.backing(parent))};
      })();
      const collect = async () => {
        for (let i = 0; i < 4; i++) {
          await new Promise(resolve => setImmediate(resolve)); global.gc();
        }
        await new Promise(resolve => setImmediate(resolve));
      };
      const released = async ref => {
        for (let i = 0; i < 8; i++) {await collect(); if (ref.deref() === undefined) return;}
        assert.fail('GC witness still reachable');
      };
      await released(parentRef);
      assert.notEqual(sourceRef.deref(), undefined, 'live child lost its source');
      assert.equal(api.get(child), 7);
      child = null;
      await released(sourceRef);
      assert.equal(api.read(copy), 7, 'detached bytes did not survive source collection');
    "#]).arg(path).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
