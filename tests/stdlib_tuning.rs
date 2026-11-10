use std::process::Command;

#[test]
fn unescaped_json_strings_do_not_call_escape_helpers() {
    let source = r#"(module
      (@use "walz:core" (func $json.stringify) (func $json.from-string))
      (func $clean (export "clean") (result i32)
        (@string.byte-len (call $json.stringify (call $json.from-string (@string.lit "abc日本語🦀/XYZ")))))
      (func $escaped (export "escaped") (result i32)
        (@string.byte-len (call $json.stringify (call $json.from-string (@string.lit "a\22b"))))))"#;
    let compiled = walz::compile(source).unwrap();
    let width = compiled
        .ir
        .as_ref()
        .unwrap()
        .functions
        .iter()
        .find(|f| f.name.ends_with(".json-string.width"))
        .unwrap()
        .name
        .clone();
    let module = compiled.ir.as_ref().unwrap();
    let builder = module
        .functions
        .iter()
        .find(|f| f.name.ends_with(".string-builder.new"))
        .unwrap();
    let builder_type = module.type_name(&builder.result);
    // Count actual escape-width calls, including calls from put. This is a
    // work budget rather than a wall-time threshold; UTF-8 checks stay enabled.
    let mut wat = compiled
        .wat
        .replace(&format!("(call {width} "), "(call $__count_width ");
    wat = wat.replace(
        &format!("(call {})", builder.name),
        "(call $__count_builder)",
    );
    wat.truncate(wat.rfind(')').unwrap());
    wat.push_str(&format!(
        r#"
      (global $__width_count (export "width-count") (mut i32) (i32.const 0))
      (global $__builder_count (export "builder-count") (mut i32) (i32.const 0))
      (func $__count_width (param i32) (result i32)
        (global.set $__width_count (i32.add (global.get $__width_count) (i32.const 1)))
        (call {width} (local.get 0)))
      (func $__count_builder (result {builder_type})
        (global.set $__builder_count (i32.add (global.get $__builder_count) (i32.const 1)))
        (call {})))"#,
        builder.name
    ));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("budget.wasm");
    std::fs::write(&path, wat::parse_str(wat).unwrap()).unwrap();
    let output = Command::new("node").args(["--input-type=module", "-e", r#"
      import assert from 'node:assert/strict';
      import { readFile } from 'node:fs/promises';
      const { instance: { exports: api } } = await WebAssembly.instantiate(await readFile(process.argv[1]));
      assert.equal(api.clean(), Buffer.byteLength('abc日本語🦀/XYZ') + 2);
      assert.equal(api['width-count'].value, 0, 'clean text should bypass escape-specific work');
      assert.equal(api.escaped(), 6);
      assert.ok(api['width-count'].value > 0);
      assert.equal(api['builder-count'].value, 0, 'standalone JSON strings should not allocate an intermediate builder');
    "#]).arg(path).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
