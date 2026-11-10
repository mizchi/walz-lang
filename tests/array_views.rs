// Compiler scope fixtures use private expanded forms; public files use @use paths.
use std::{collections::BTreeSet, process::Command};
use walz::ir::{Expr, ExprKind, FunctionKind, Module};

const SOURCE: &str = r#"(module
  (@use "walz:core" (type $list) (type $array-view)
    (func $array-view.new) (func $array-view.len) (func $array-view.get)
    (func $array-view.slice) (func $array-view.to-list))
  (type $Numbers (array (mut i64)))
  (@type.alias $View (ref (@inst $array-view i64 $Numbers)))
  (func $scan (param $source (ref $Numbers)) (result i64) (@effect reads allocates)
    (@let (($whole (call (@inst $array-view.new i64 $Numbers) $source))
           ($part (call (@inst $array-view.slice i64 $Numbers) $whole 1 3))
           ($tail (call (@inst $array-view.slice i64 $Numbers) $part 1 2)))
      (+ (call (@inst $array-view.get i64 $Numbers) $tail 0)
         (call (@inst $array-view.get i64 $Numbers) $tail 1))))
  (func $main (export "main") (result i64) (@effect reads allocates)
    (call $scan (array.new_fixed $Numbers 5 99 1 40 2 99)))
  (func $source (export "source") (result (ref $Numbers)) (@effect allocates)
    (array.new $Numbers 42 65536))
  (func $new (export "new") (param $source (ref $Numbers)) (result $View) (@effect allocates)
    (call (@inst $array-view.new i64 $Numbers) $source))
  (func $slice (export "slice") (param $v $View) (param $start i32) (param $length i32)
    (result $View) (@effect allocates)
    (call (@inst $array-view.slice i64 $Numbers) $v $start $length))
  (func $length (export "length") (param $v $View) (result i32) (@effect pure)
    (call (@inst $array-view.len i64 $Numbers) $v))
  (func $get (export "get") (param $v $View) (param $index i32) (result i64) (@effect reads)
    (call (@inst $array-view.get i64 $Numbers) $v $index))
  (func $set (export "set") (param $source (ref $Numbers)) (param $index i32) (param $value i64)
    (@effect writes) (array.set $Numbers $source $index $value))
  (func $copy (export "copy") (param $v $View) (result (@inst $list i64)) (@effect reads writes allocates)
    (call (@inst $array-view.to-list i64 $Numbers) $v))
  (func $item (export "item") (param $xs (@inst $list i64)) (result i64) (@effect pure)
    (@list.get i64 $xs 0))
  (func $size (export "size") (param $xs (@inst $list i64)) (result i32) (@effect pure)
    (@list.len i64 $xs)))"#;

fn no_payload_copies(module: &Module, expr: &Expr, visited: &mut BTreeSet<usize>) {
    assert!(
        !matches!(
            expr.kind,
            ExprKind::ArrayNew { .. } | ExprKind::ArrayFixed { .. }
        ),
        "view allocated element storage"
    );
    if let ExprKind::Call { function, .. } = expr.kind
        && visited.insert(function.0)
        && let Some(body) = &module.functions[function.0].body
    {
        no_payload_copies(module, body, visited);
    }
    for child in expr.children() {
        no_payload_copies(module, child, visited);
    }
}

#[test]
fn array_views_share_ranges_check_bounds_and_release_parent_and_backing() {
    let module = walz::check(SOURCE).unwrap();
    let scan = module.functions.iter().find(|f| f.name == "$scan").unwrap();
    no_payload_copies(&module, scan.body.as_ref().unwrap(), &mut BTreeSet::new());
    for function in &module.functions {
        if matches!(function.kind, FunctionKind::Closure { .. }) {
            no_payload_copies(
                &module,
                function.body.as_ref().unwrap(),
                &mut BTreeSet::new(),
            );
        }
    }
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(SOURCE, walz::CompileOptions { optimize }).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("views.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let out = Command::new("node").args(["--expose-gc", "--input-type=module", "-e", r#"
          import assert from 'node:assert/strict';
          import {readFile} from 'node:fs/promises';
          const {instance} = await WebAssembly.instantiate(await readFile(process.argv[1]));
          const a = instance.exports;
          assert.equal(a.main(), 42n);
          let {child, copy, parentRef, sourceRef} = (() => {
            const source = a.source(), parent = a.new(source), child = a.slice(a.slice(parent, 3, 5), 1, 2);
            a.set(source, 4, 40n); a.set(source, 5, 2n);
            assert.equal(a.get(child, 0) + a.get(child, 1), 42n);
            assert.equal(a.length(a.slice(child, 2, 0)), 0);
            assert.equal(a.length(child), 2);
            assert.equal(a.size(a.copy(a.slice(child, 2, 0))), 0);
            const copy = a.copy(child);
            a.set(source, 4, 99n);
            assert.equal(a.item(copy), 40n, 'materialization must detach');
            assert.equal(a.get(child, 0), 99n, 'view must observe source mutations');
            for (const [start, length] of [[-1,0],[0,-1],[3,0],[1,2],[2147483647,2147483647]])
              assert.throws(() => a.slice(child, start, length), WebAssembly.RuntimeError);
            for (const index of [-1, 2, 2147483647]) assert.throws(() => a.get(child,index), WebAssembly.RuntimeError);
            return {child, copy, parentRef:new WeakRef(parent), sourceRef:new WeakRef(source)};
          })();
          const released = async ref => {
            for(let n=0;n<16;n++) {
              await new Promise(r=>setImmediate(r)); global.gc();
              await new Promise(r=>setImmediate(r));
              if(ref.deref()===undefined)return;
            }
            assert.fail('GC witness retained');
          };
          await released(parentRef);
          assert.notEqual(sourceRef.deref(),undefined,'live child must root backing');
          assert.equal(a.get(child,0),99n);
          child=null;
          await released(sourceRef);
          assert.equal(a.item(copy),40n);
        "#]).arg(path).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn constructor_rejects_wrong_elements_before_get_is_used_and_views_remain_opaque() {
    let source = r#"(module (@use "walz:core" (type $array-view) (func $array-view.new))
      (type $Numbers (array (mut i64)))
      (func $new (param $source (ref $Numbers)) (result (ref (@inst $array-view i32 $Numbers)))
        (call (@inst $array-view.new i32 $Numbers) $source)))"#;
    assert!(
        walz::check(source)
            .unwrap_err()
            .to_string()
            .contains("type mismatch")
    );
    let opaque = r#"(module (@use "walz:core" (type $array-view))
      (type $Numbers (array (mut i64)))
      (func $bad (param $v (ref (@inst $array-view i64 $Numbers))) (result i32)
        (struct.get (@inst $array-view i64 $Numbers) $length $v)))"#;
    assert_eq!(
        walz::check(opaque).unwrap_err().diagnostic().code,
        "E_OPAQUE_REPRESENTATION"
    );
    assert_eq!(
        walz::check(&SOURCE.replace("(@effect reads)", "(@effect pure)"))
            .unwrap_err()
            .diagnostic()
            .code,
        "E_EFFECT_CONTRACT"
    );
}

#[test]
fn immutable_arrays_nondefaultable_references_and_simd_elements_materialize() {
    for (element, value, read) in [
        (
            "i64",
            "(i64.const 42)",
            "(i32.wrap_i64 (@list.get i64 $copy 0))",
        ),
        (
            "$string",
            "(@string.lit \"*hello\")",
            "(@bytes.get (@string.bytes (@list.get $string $copy 0)) 0)",
        ),
        (
            "(ref $Point)",
            "(struct.new $Point 42)",
            "(struct.get $Point $x (@list.get (ref $Point) $copy 0))",
        ),
        (
            "v128",
            "(v128.const i32x4 42 0 0 0)",
            "(i32x4.extract_lane 0 (@list.get v128 $copy 0))",
        ),
    ] {
        let source = format!(
            r#"(module
          (@use "walz:core" (type $list) (type $string)
            (func $array-view.new) (func $array-view.to-list))
          (type $Point (struct (field $x i32)))
          (type $Items (array {element}))
          (func $main (export "main") (result i32) (@effect reads writes allocates)
            (@let (($source (array.new_fixed $Items 1 {value}))
                   ($view (call (@inst $array-view.new {element} $Items) $source))
                   ($copy (call (@inst $array-view.to-list {element} $Items) $view))
                   ($empty (call (@inst $array-view.to-list {element} $Items)
                     (call (@inst $array-view.new {element} $Items) (array.new_fixed $Items 0)))))
              (+ (@list.len {element} $empty) {read}))))"#
        );
        for optimize in [false, true] {
            let compiled =
                walz::compile_with_options(&source, walz::CompileOptions { optimize }).unwrap();
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("elements.wasm");
            std::fs::write(&path, compiled.wasm).unwrap();
            let out = Command::new("wasmtime")
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
                out.status.success(),
                "{element}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                "42",
                "{element}"
            );
        }
    }
}

#[test]
fn source_kind_bounds_and_public_abi_are_checked() {
    let structure = r#"(module (@use "walz:core" (type $array-view) (func $array-view.new))
      (type $Point (struct (field i64)))
      (func $bad (param $source (ref $Point)) (result (ref (@inst $array-view i64 $Point)))
        (call (@inst $array-view.new i64 $Point) $source)))"#;
    assert!(
        walz::check(structure)
            .unwrap_err()
            .to_string()
            .contains("array")
    );
    let opaque_source = r#"(module
      (@use "walz:core" (func $array-view.new))
      (@legacy.namespace $secret
        (@export (opaque $Numbers) (func $new))
        (type $Numbers (array (mut i64)))
        (func $new (result (ref $Numbers)) (array.new_fixed $Numbers 1 42)))
      (func $bad
        (drop (call (@inst $array-view.new i64 $secret.Numbers) (call $secret.new)))))"#;
    assert_eq!(
        walz::check(opaque_source).unwrap_err().diagnostic().code,
        "E_OPAQUE_REPRESENTATION"
    );
    for argument in ["$unit", "(@inst $future i64)"] {
        let source = format!(
            r#"(module
          (@use "walz:core" (type $unit) (type $future) (type $array-view) (func $array-view.new))
          (type $Numbers (array (mut i64)))
          (func $bad (param $source (ref $Numbers)) (result (ref (@inst $array-view {argument} $Numbers)))
            (call (@inst $array-view.new {argument} $Numbers) $source)))"#
        );
        assert_eq!(
            walz::check(&source).unwrap_err().diagnostic().code,
            "E_GENERIC_BOUND"
        );
    }
    let module = walz::check(
        r#"(module
      (@use "walz:core" (type $array-view) (func $array-view.new))
      (type $Numbers (array (mut i64)))
      (func $view (export "view") (result (ref (@inst $array-view i64 $Numbers)))
        (call (@inst $array-view.new i64 $Numbers) (array.new_fixed $Numbers 1 42))))"#,
    )
    .unwrap();
    assert!(
        walz::wit::emit(&module)
            .unwrap_err()
            .to_string()
            .contains("GC references")
    );
}
