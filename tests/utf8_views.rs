use std::collections::BTreeSet;
use walz::ir::{Expr, ExprKind, Module};

const VALIDATE: &str = r#"(module
  (@use "walz:core" (type $bytes)
    (func $bytes-view.new) (func $bytes-view.slice)
    (func $string-view.from-bytes-view) (func $string-view.byte-len))
  (func $validate (export "validate") (param $b $bytes) (param $start i32) (param $n i32)
    (result i32) (@effect allocates)
    (@match (call $string-view.from-bytes-view
      (call $bytes-view.slice (call $bytes-view.new $b) $start $n))
      (@case none () -1)
      (@case some ($v) (call $string-view.byte-len $v)))))"#;

fn no_payload_copy(module: &Module, e: &Expr, visited: &mut BTreeSet<usize>) {
    match &e.kind {
        ExprKind::Buffer { op, .. } => assert!(!op.allocates(), "payload operation: {op:?}"),
        ExprKind::ArrayNew { .. } | ExprKind::ArrayFixed { .. } | ExprKind::Data(_) => {
            panic!("payload allocation")
        }
        ExprKind::Call { function, .. } if visited.insert(function.0) => {
            if let Some(body) = &module.functions[function.0].body {
                no_payload_copy(module, body, visited);
            }
        }
        _ => {}
    }
    for child in e.children() {
        no_payload_copy(module, child, visited);
    }
}

#[test]
fn bytes_views_are_validated_without_copying_or_materializing_a_string() {
    let module = walz::check(VALIDATE).unwrap();
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "$validate")
        .unwrap();
    no_payload_copy(&module, f.body.as_ref().unwrap(), &mut BTreeSet::new());
    walz::verified::compile(VALIDATE).unwrap();
    let source = format!(
        r#"{}
      (func $main (export "main") (result i32) (@effect allocates)
        (call $validate (@bytes.new 255 240 159 166 128 255) 1 4)))"#,
        VALIDATE.trim_end().strip_suffix(')').unwrap()
    );
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(&source, walz::CompileOptions { optimize }).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("range.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let output = std::process::Command::new("wasmtime")
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
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "4");
    }
}

#[test]
fn utf8_ranges_have_checked_types_and_an_explicit_materialization_boundary() {
    let source = r#"(module
      (@use "walz:core" (type $bytes) (type $string) (type $option))
      (func $valid (param $b $bytes) (param $p i32) (param $n i32) (result i32)
        (@effect pure) (@bytes.is-utf8 $b $p $n))
      (func $decode (param $b $bytes) (param $p i32) (param $n i32)
        (result (@inst $option $string)) (@effect pure) (@string.from-bytes $b $p $n)))"#;
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        assert!(compiled.ir.is_some());
        assert!(!compiled.wat.contains("(memory "));
    }
}

#[test]
fn ranges_and_view_construction_keep_type_effect_ownership_and_opacity_checks() {
    for body in [
        "(@bytes.is-utf8)",
        "(@bytes.is-utf8 (@bytes.new) 0)",
        "(@bytes.is-utf8 (@bytes.new) 0 0 0)",
        "(@bytes.is-utf8 (@bytes.new) (i64.const 0) 0)",
        "(@bytes.is-utf8 (@string.lit \"x\"))",
    ] {
        assert!(
            walz::check(&format!("(module (func $f (result i32) {body}))")).is_err(),
            "{body}"
        );
    }
    for body in [
        "(@string.from-bytes)",
        "(@string.from-bytes (@bytes.new) 0)",
        "(@string.from-bytes (@bytes.new) (i64.const 0) 0)",
    ] {
        assert!(walz::check(&format!("(module (@use \"walz:core\" (type $option) (type $string)) (func $f (result (@inst $option $string)) {body}))")).is_err(), "{body}");
    }
    assert_eq!(
        walz::check(&VALIDATE.replace("(@effect allocates)", "(@effect pure)"))
            .unwrap_err()
            .diagnostic()
            .code,
        "E_EFFECT_CONTRACT"
    );
    let source = walz::source::load("tests/component-fixtures/utf8-views.walz").unwrap();
    walz::verified::compile(source.text()).unwrap();
    let watz = walz::review::to_watz(source.text()).unwrap();
    walz::compile(&walz::review::from_watz(&watz).unwrap().text).unwrap();
    // There is no public unchecked construction of validated string views.
    assert_eq!(
        walz::check(
            r#"(module
      (@use "walz:core" (type $bytes-view) (type $string-view))
      (func $bad (param $v (ref $bytes-view)) (result (ref $string-view))
        (struct.new $string-view $v)))"#
        )
        .unwrap_err()
        .diagnostic()
        .code,
        "E_OPAQUE_REPRESENTATION"
    );
    assert!(
        walz::check(
            r#"(module
      (@use "walz:core" (type $bytes-view) (type $string-view) (type $option)
        (func $string-view.from-bytes-view))
      (func $bad (param $v (ref null $bytes-view)) (result (@inst $option (ref $string-view)))
        (call $string-view.from-bytes-view $v)))"#
        )
        .is_err()
    );
    assert!(
        walz::check(
            r#"(module
      (@use "walz:core" (type $bytes) (type $box))
      (func $bad (param $v (@inst $box $bytes)) (result i32)
        (@bytes.is-utf8 (@move $v))))"#
        )
        .is_err()
    );
    for (result, body) in [
        ("i32", "(@bytes.is-utf8 (unreachable))"),
        ("i32", "(@bytes.is-utf8 (@bytes.new) (unreachable) 0)"),
        (
            "(@inst $option $string)",
            "(@string.from-bytes (unreachable) 0 0)",
        ),
        (
            "(@inst $option $string)",
            "(@string.from-bytes (@bytes.new) 0 (unreachable))",
        ),
    ] {
        walz::compile(&format!("(module (@use \"walz:core\" (type $option) (type $string)) (func $f (result {result}) {body}))")).unwrap();
    }
}

#[test]
fn validated_views_share_storage_keep_it_live_and_materialized_strings_detach() {
    let source = r#"(module
      (@use "walz:core" (type $string) (type $bytes-view) (type $string-view)
        (func $bytes-view.new) (func $bytes-view.slice)
        (func $string-view.from-bytes-view) (func $string-view.slice)
        (func $string-view.byte-len) (func $string-view.to-string))
      (type $Storage (array (mut i8)))
      (func $make (export "make") (result (ref $bytes-view))
        (@let $a (array.new $Storage 65 131072)
          (block (result (ref $bytes-view))
            (array.set $Storage $a 0 255)
            (call $bytes-view.new (@bytes.from-array $Storage $a)))))
      (func $validate (export "validate") (param $v (ref $bytes-view)) (result (ref $string-view))
        (@match (call $string-view.from-bytes-view (call $bytes-view.slice $v 1 8))
          (@case none () (unreachable)) (@case some ($text) $text)))
      (func $child (export "child") (param $v (ref $string-view)) (result (ref $string-view))
        (call $string-view.slice $v 2 4))
      (func $length (export "length") (param $v (ref $string-view)) (result i32)
        (call $string-view.byte-len $v))
      (func $copy (export "copy") (param $v (ref $string-view)) (result $string)
        (call $string-view.to-string $v))
      (func $read (export "read") (param $s $string) (result i32) (@string.byte-len $s)))"#;
    let compiled = walz::compile(source).unwrap();
    let module = compiled.ir.as_ref().unwrap();
    let view = &module
        .types
        .iter()
        .find(|t| t.name.ends_with(".bytes-view"))
        .unwrap()
        .name;
    let string_view = &module
        .types
        .iter()
        .find(|t| t.name.ends_with(".string-view"))
        .unwrap()
        .name;
    let bytes = &module
        .types
        .iter()
        .find(|t| {
            matches!(
                t.kind,
                walz::ir::TypeKind::Value(walz::ir::ValueType::Bytes)
            )
        })
        .unwrap()
        .name;
    let text = &module
        .types
        .iter()
        .find(|t| matches!(t.kind, walz::ir::TypeKind::Value(walz::ir::ValueType::Text)))
        .unwrap()
        .name;
    // Test-only observers; production source cannot access these representations.
    let observers = format!(
        r#"
      (func (export "owner") (param (ref {view})) (result anyref)
        (struct.get {bytes} 0 (struct.get {view} 0 (local.get 0))))
      (func (export "backing") (param (ref {string_view})) (result anyref)
        (struct.get {bytes} 0 (struct.get {view} 0 (struct.get {string_view} 0 (local.get 0)))))
      (func (export "copyBacking") (param (ref {text})) (result anyref)
        (struct.get {text} 0 (local.get 0))))"#
    );
    let wat = compiled.wat.trim_end();
    let wasm = wat::parse_str(format!("{}{observers}", &wat[..wat.len() - 1])).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("views.wasm");
    std::fs::write(&path, wasm).unwrap();
    let output = std::process::Command::new("node")
        .args([
            "--expose-gc",
            "--input-type=module",
            "-e",
            r#"
      import assert from 'node:assert/strict';
      import {readFile} from 'node:fs/promises';
      const {instance:{exports:e}}=await WebAssembly.instantiate(await readFile(process.argv[1]));
      let {child,copy,parentRef,sourceRef} = (()=>{
        const source=e.make(), parent=e.validate(source), child=e.child(parent), copy=e.copy(child);
        assert.equal(e.owner(source),e.backing(parent),'validation copied bytes');
        assert.equal(e.backing(parent),e.backing(child),'slice copied bytes');
        assert.notEqual(e.backing(child),e.copyBacking(copy),'materialization retained owner');
        return {child,copy,parentRef:new WeakRef(parent),sourceRef:new WeakRef(e.owner(source))};
      })();
      const released=async ref=>{
        for(let i=0;i<40;i++){
          await new Promise(r=>setImmediate(r));global.gc();await new Promise(r=>setImmediate(r));
          if(ref.deref()===undefined)return;
        }
        assert.fail('unexpected GC root');
      };
      await released(parentRef);
      assert.notEqual(sourceRef.deref(),undefined,'live view lost backing storage');
      assert.equal(e.length(child),4);
      child=null;
      await released(sourceRef);
      assert.equal(e.read(copy),4,'detached string lost after source collection');
    "#,
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
