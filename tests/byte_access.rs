#[test]
fn fixed_width_access_is_typed_checked_and_uses_gc_storage() {
    let source = r#"(module
      (@use "walz:core" (type $bytes))
      (type $Storage (array (mut i8)))
      (func $read (export "read") (param $b $bytes) (param $p i32) (result i32)
        (@effect pure) (@bytes.read-i32-le $b $p))
      (func $write (export "write") (param $a (ref $Storage)) (param $p i32) (param $v i32)
        (@effect writes) (@array.write-i32-le $Storage $a $p $v)))"#;
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        assert!(compiled.ir.is_some());
        assert!(!compiled.wat.contains("(memory "));
    }
    walz::verified::compile(source).unwrap();
}

#[test]
fn core_bytes_and_view_readers_are_explicitly_importable() {
    walz::verified::compile(
        r#"(module
      (@use "walz:core" (type $bytes) (type $bytes-view)
        (func $bytes.read-i32-le) (func $bytes-view.new) (func $bytes-view.read-i32-be))
      (func $f (export "f") (param $b $bytes) (result i32) (@effect allocates)
        (+ (call $bytes.read-i32-le $b 0)
           (call $bytes-view.read-i32-be (call $bytes-view.new $b) 1))))"#,
    )
    .unwrap();
}

#[test]
fn formats_operand_types_storage_and_effects_are_checked() {
    let source = walz::source::load("tests/component-fixtures/byte-access.walz").unwrap();
    walz::verified::compile(source.text()).unwrap();
    walz::component::compile(source.text()).unwrap();
    let watz = walz::review::to_watz(source.text()).unwrap();
    let translated = walz::review::from_watz(&watz).unwrap();
    walz::compile(&translated.text).unwrap();
    for body in [
        "(@bytes.read-i32-le (i32.const 0) 0)",
        "(@bytes.read-i32-le (@bytes.new 0 0 0 0) (i64.const 0))",
        "(@bytes.read-i32-native (@bytes.new 0 0 0 0) 0)",
        "(@bytes.read-i32-le (@bytes.new 0 0 0 0))",
        "(@bytes.read-i64-le (@bytes.new) 0)", // wrong result type
    ] {
        assert!(
            walz::check(&format!("(module (func $f (result i32) {body}))")).is_err(),
            "{body}"
        );
    }
    for storage in ["i8", "(mut i16)", "(mut i32)", "(mut i64)"] {
        let source = format!(
            "(module (type $A (array {storage})) (func $f (param $a (ref $A)) (@array.write-i32-le $A $a 0 1)))"
        );
        assert!(walz::check(&source).is_err(), "{storage}");
    }
    for (parameter, body) in [
        ("(ref null $A)", "(@array.write-i32-le $A $a 0 1)"),
        ("(ref $A)", "(@array.write-i64-le $A $a 0 (i32.const 1))"),
        ("(ref $A)", "(@array.write-f32-be $A $a 0 (f64.const 1))"),
        ("(ref $A)", "(@array.write-i32-le $A $a (i64.const 0) 1)"),
        ("(ref $A)", "(@array.write-i32-le $A $a 0)"),
        ("(ref $A)", "(@array.write-i32-native $A $a 0 1)"),
    ] {
        let source =
            format!("(module (type $A (array (mut i8))) (func $f (param $a {parameter}) {body}))");
        assert!(walz::check(&source).is_err(), "{body}");
    }
    let error = walz::check("(module (type $A (array (mut i8))) (func $f (param $a (ref $A)) (@effect pure) (@array.write-i32-le $A $a 0 1)))").unwrap_err();
    assert_eq!(error.diagnostic().code, "E_EFFECT_CONTRACT");
    assert!(error.to_string().contains("fixed-width array write"));
    // The extension cannot be silently discarded as an ordinary WAT annotation.
    assert!(walz::check("(module (type $A (array (mut i8))) (func $f (param $a (ref $A)) (@array.write-i32-le $A $a 0 (i64.const 1))))").is_err());
    for body in [
        "(@bytes.read-i32-le (unreachable) 0)",
        "(@bytes.read-i32-le (@bytes.new) (unreachable))",
    ] {
        walz::compile(&format!("(module (func $f (result i32) {body}))")).unwrap();
    }
    for body in [
        "(@array.write-i32-le $A (unreachable) 0 1)",
        "(@array.write-i32-le $A $a (unreachable) 1)",
        "(@array.write-i32-le $A $a 0 (unreachable))",
    ] {
        walz::compile(&format!(
            "(module (type $A (array (mut i8))) (func $f (param $a (ref $A)) {body}))"
        ))
        .unwrap();
    }
}

#[test]
fn writes_are_reported_against_the_destination_without_allocation_or_escape() {
    let module = walz::check("(module (type $A (array (mut i8))) (func $f (param $a (ref $A)) (@effect writes) (@array.write-f64-be $A $a 0 1.5)))").unwrap();
    let analysis = serde_json::to_value(walz::analysis::analyze(&module)).unwrap();
    let function = &analysis["functions"][0];
    assert_eq!(
        function["memory"]["writes"][0]["object"]["root"]["kind"],
        "parameter"
    );
    assert!(function["memory"]["reads"].as_array().unwrap().is_empty());
    assert!(
        function["memory"]["allocations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let body = module.functions[0].body.as_ref().unwrap();
    let walz::ir::ExprKind::Buffer { op, .. } = body.kind else {
        panic!("missing typed operation")
    };
    assert!(op.may_trap());
    assert!(!op.allocates());
}

#[test]
fn opaque_arrays_remain_private_and_imported_macros_retain_definition_scope() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("secret.walz"),
        r#"(module
      (type $Storage (array (mut i8)))
      (@export (opaque $Storage) (macro $write))
      (@macro $write (rule (?a:expr)
        (@array.write-i32-le $Storage ?a 0 42))))"#,
    )
    .unwrap();
    let main = directory.path().join("main.walz");
    std::fs::write(
        &main,
        r#"(module
      (@use "./secret.walz" (type $Storage))
      (func $f (param $a (ref $Storage)) (@array.write-i32-le $Storage $a 0 42)))"#,
    )
    .unwrap();
    let source = walz::source::load(&main).unwrap();
    assert_eq!(
        walz::check(source.text()).unwrap_err().diagnostic().code,
        "E_OPAQUE_REPRESENTATION"
    );
    // Macro expansion cannot bypass opaque representation checks either.
    std::fs::write(
        &main,
        r#"(module
      (@use "./secret.walz" (type $Storage $Hidden) (macro $write))
      (type $Storage (array (mut i64)))
      (func $f (param $a (ref $Hidden)) (@effect writes) (@$write $a)))"#,
    )
    .unwrap();
    let source = walz::source::load(&main).unwrap();
    assert_eq!(
        walz::check(source.text()).unwrap_err().diagnostic().code,
        "E_OPAQUE_REPRESENTATION"
    );
    let secret = directory.path().join("secret.walz");
    let public = std::fs::read_to_string(&secret)
        .unwrap()
        .replace("(opaque $Storage)", "(type $Storage)");
    std::fs::write(secret, public).unwrap();
    // With public storage, the caller's identically spelled i64 type must not
    // capture the macro's nominal i8 type operand.
    let source = walz::source::load(&main).unwrap();
    walz::verified::compile(source.text()).unwrap();
}

#[test]
fn invalid_writes_leave_storage_unchanged_and_operands_precede_bounds_checks() {
    let mut source = r#"(module
      (type $A (array (mut i8)))
      (tag $Stop)
      (func $make (export "make") (result (ref $A)) (@effect allocates) (array.new $A 173 8))
      (func $get (export "get") (param $a (ref $A)) (param $i i32) (result i32) (@effect reads) (array.get_u $A $a $i))
      (func $write (export "write") (param $a (ref $A)) (param $p i32) (@effect writes) (@array.write-i64-be $A $a $p -1))
      (func $throw (result i32) (@effect (throws $Stop)) (throw $Stop))
      (func $order (export "order") (param $a (ref $A)) (result i32) (@effect writes)
        (@try (result i32)
          (do (block (result i32) (@array.write-i32-le $A $a -1 (call $throw)) 0))
          (catch $Stop () 42)))"#.to_owned();
    for (format, integer) in [("f32", "i32"), ("f64", "i64")] {
        for order in ["le", "be"] {
            source.push_str(&format!(
                r#"
              (func $bits-{format}-{order} (export "bits-{format}-{order}")
                (param $bits {integer}) (result {integer}) (@effect reads writes allocates)
                (@let $a (array.new $A 0 9)
                  (block (result {integer})
                    (@array.write-{format}-{order} $A $a 1 ({format}.reinterpret_{integer} $bits))
                    ({integer}.reinterpret_{format}
                      (@bytes.read-{format}-{order} (@bytes.from-array $A $a) 1)))))"#
            ));
        }
    }
    source.push(')');
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(&source, walz::CompileOptions { optimize }).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let wasm = directory.path().join("write.wasm");
        std::fs::write(&wasm, compiled.wasm).unwrap();
        let output = std::process::Command::new("node").args(["--input-type=module", "-e", r#"
          import assert from 'node:assert/strict';
          import {readFile} from 'node:fs/promises';
          const {instance:{exports:e}}=await WebAssembly.instantiate(await readFile(process.argv[1]));
          const a=e.make();
          for(const p of [-1,1,7,8,2147483647]) {
            assert.throws(()=>e.write(a,p),WebAssembly.RuntimeError);
            for(let i=0;i<8;i++)assert.equal(e.get(a,i),173,'partial write before trap');
          }
          assert.equal(e.order(a),42,'value exception must precede range trap');
          for(let i=0;i<8;i++)assert.equal(e.get(a,i),173);
          e.write(a,0);for(let i=0;i<8;i++)assert.equal(e.get(a,i),255);
          // Stay inside Wasm's integer boundary to observe NaN payloads exactly.
          for(const order of ['le','be']) {
            for(const bits of [0,0x80000000,0x7fc12345,0x7f812345,0xffc12345,1,0xffffffff])
              assert.equal(e['bits-f32-'+order](bits),bits|0);
            for(const bits of [0n,1n,0x8000000000000000n,0x7ff8123456789abcn,0x7ff0123456789abcn,0xfff8123456789abcn])
              assert.equal(e['bits-f64-'+order](bits),BigInt.asIntN(64,bits));
          }
        "#]).arg(&wasm).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
