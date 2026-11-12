#[test]
fn private_values_are_scalarized_across_locals_branches_and_small_call_returns() {
    for expression in [
        "(@let $unused (@option.some i32 7) 42)",
        "(call $consume (@option.some i32 42))",
        "(@match (@option i32) (result i32) (@option.some i32 42) (@case none () 0) (@case some ($n) $n))",
        "(@let $v (@result.ok i32 i32 42) (@match $v (@case ok ($n) $n) (@case err ($e) $e)))",
        "(@let $v (@tuple.new i32 i32 20 1) (+ (@match $v (@case pair ($a $b) (+ $a $b))) (@match $v (@case pair ($a $b) (+ $a $b)))))",
        "(@match (call $choose $flag) (@case none () 42) (@case some ($n) $n))",
        "(@let $v (if (result (@option i32)) $flag (then (@option.some i32 42)) (else (@option.none i32))) (@match $v (@case none () 42) (@case some ($n) $n)))",
    ] {
        let source = format!(
            r#"(module
          (func $consume (param $v (@option i32)) (result i32)
            (@match $v (@case none () 0) (@case some ($n) $n)))
          (func $choose (param $flag i32) (result (@option i32))
            (if (result (@option i32)) $flag (then (@option.some i32 42)) (else (@option.none i32))))
          (func $main (export "main") (param $flag i32) (result i32) {expression}))"#
        );
        let original =
            walz::compile_with_options(&source, walz::CompileOptions { optimize: false }).unwrap();
        let optimized = walz::compile(&source).unwrap();
        assert!(original.wat.contains("struct.new $__variant"));
        assert!(
            !optimized.wat.contains("struct.new $__variant"),
            "{expression}\n{}",
            optimized.wat
        );
        assert!(!optimized.wat.contains("ref.cast"), "{}", optimized.wat);
    }
}

#[test]
fn default_payload_evaluation_is_retained_when_the_container_disappears() {
    let source = r#"(module (func $main (export "main") (result i32)
      (@match (@result.ok i32 i32 (i32.div_s 1 0)) (@default 42))))"#;
    assert!(
        walz::compile(source).unwrap().wat.contains("i32.div_s"),
        "unused payload must still trap"
    );
}

#[test]
fn escaped_owned_and_identity_bearing_containers_keep_their_representation() {
    for source in [
        "(module (func $main (export \"main\") (result (@option i32)) (@option.some i32 42)))",
        "(module (func $main (export \"main\") (@let $v (@option.some (@box i32) (@box.new i32 42)) (@match (@move $v) (@case none () (nop)) (@case some ($owner) (@box.close (@move $owner)))))))",
        "(module (type $V (@variant (yes i32))) (func $main (export \"main\") (result i32) (@match $V (result i32) (@variant.new $V yes 42) (@case yes ($n) $n))))",
    ] {
        assert!(walz::compile(source).unwrap().wat.contains("struct.new"));
    }
}

#[test]
fn optimized_and_original_values_agree_on_effects_exceptions_traps_and_boundaries() {
    let fixture = include_str!("component-fixtures/value-optimization.walz");
    let prefix = fixture.trim_end().strip_suffix(')').unwrap();
    let source = format!(
        r#"{prefix}
      (import "env" "mark" (func $mark (param $digit i32) (result i32)))
      (func $order (export "order") (result i32)
        (@let $v (@tuple.new i32 i32 (call $mark 1) (call $mark 2))
          (block (result i32) (drop (call $mark 3))
            (+ (@match $v (@case pair ($a $b) (+ $a $b)))
               (@match $v (@case pair ($a $b) (+ $a $b)))))))
      (func $discard (export "discard") (result i32)
        (@match (@tuple.new i32 i32 (call $mark 1) (call $mark 2)) (@default (call $mark 3))))
      (func $maybe (param $n i32) (param $unused i32) (result (@option i32)) (@option.some $n))
      (func $unused (export "unused") (result i32)
        (@match (call $maybe (call $mark 1) (call $mark 2)) (@case none () 0) (@case some ($n) $n)))
      (func $recursive (param $n i32) (result (@option i32))
        (if (result (@option i32)) (i32.eqz $n)
          (then (@option.some 42)) (else (call $recursive (- $n 1)))))
      (func $rec (export "rec") (result i32)
        (@match (call $recursive 100) (@case none () 0) (@case some ($n) $n))))"#
    );
    let directory = tempfile::tempdir().unwrap();
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(&source, walz::CompileOptions { optimize }).unwrap();
        if optimize {
            for name in [
                "branch",
                "layout",
                "converted",
                "order",
                "discard",
                "unused",
            ] {
                let body = compiled
                    .wat
                    .split(&format!("(func ${name} "))
                    .nth(1)
                    .unwrap()
                    .split("\n  (func ")
                    .next()
                    .unwrap();
                assert!(!body.contains("struct.new $__variant"), "{name}: {body}");
            }
        }
        std::fs::write(
            directory
                .path()
                .join(if optimize { "on.wasm" } else { "off.wasm" }),
            compiled.wasm,
        )
        .unwrap();
    }
    let output = std::process::Command::new("node").args(["--input-type=module", "-e", r#"
      import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
      let trace=[],fail=0;
      const env={mark:n=>{trace.push(n);if(n===fail)throw new Error('marker '+n);return n;}};
      const [off,on]=await Promise.all(['off','on'].map(async name=>(await WebAssembly.instantiate(await readFile(process.argv[1]+'/'+name+'.wasm'),{env})).instance.exports));
      const outcome=fn=>{try{return ['value',fn()];}catch(e){return ['error',e.constructor.name,e.message.startsWith('marker ')?e.message:'wasm trap'];}};
      for(const n of [-2147483648,-1,0,1,2,65535,65536,2147483647])for(const flag of [-1,0,1]){
        for(const name of ['branch','converted'])assert.deepEqual(outcome(()=>on[name](flag,n)),outcome(()=>off[name](flag,n)));
        for(const b of [-1,0,1,4,2147483647])for(const h of [-1,0,2])assert.equal(on.layout(n,b,h),off.layout(n,b,h));
      }
      for(const name of ['identity','handler','rec','main'])assert.deepEqual(outcome(()=>on[name]()),outcome(()=>off[name]()));
      for(const n of [-1,0,1,2])for(const name of ['unit','null','caught','ignored'])assert.deepEqual(outcome(()=>on[name](n)),outcome(()=>off[name](n)));
      for(const name of ['order','discard','unused'])for(fail of [0,1,2,3]){
        trace=[];const original=outcome(()=>off[name]()),before=trace;
        trace=[];assert.deepEqual(outcome(()=>on[name]()),original,name);assert.deepEqual(trace,before,name+' evaluation order');
      }
    "#]).arg(directory.path()).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn external_base_values_and_valid_tags_still_require_subtype_checks() {
    let source = r#"(module (func $read (export "read") (param $v (@option i32)) (result i32)
      (@match $v (@case none () 0) (@default 42))))"#;
    for optimize in [false, true] {
        let mut text = walz::compile_with_options(source, walz::CompileOptions { optimize })
            .unwrap()
            .wat;
        text.truncate(text.rfind(')').unwrap());
        text.push_str(r#"(func (export "forged") (result i32) (call $read (struct.new $__value0 (i32.const 1)))))"#);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("forged.wasm");
        std::fs::write(&path, wat::parse_str(text).unwrap()).unwrap();
        let output = std::process::Command::new("node").args(["--input-type=module", "-e", "import assert from 'node:assert/strict';import{readFile}from'node:fs/promises';const{instance}=await WebAssembly.instantiate(await readFile(process.argv[1]));assert.throws(()=>instance.exports.forged(),WebAssembly.RuntimeError);"]).arg(path).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn excessive_continuation_duplication_keeps_the_original_boxed_form() {
    let mut producer = "(@option.some i32 42)".to_string();
    for bit in 0..6 {
        producer = format!(
            "(if (result (@option i32)) (i32.and $flag {}) (then {producer}) (else {producer}))",
            1 << bit
        );
    }
    let body = "(drop (+ $n 1))".repeat(96);
    let source = format!(
        "(module (func $f (export \"f\") (param $flag i32) (result i32) (@let $v {producer} (@match $v (@case none () 0) (@case some ($n) (block (result i32) {body} $n))))))"
    );
    let original =
        walz::compile_with_options(&source, walz::CompileOptions { optimize: false }).unwrap();
    let optimized = walz::compile(&source).unwrap();
    assert!(optimized.wat.contains("struct.new $__variant"));
    assert_eq!(
        original.wat, optimized.wat,
        "declined rewrites must not leave partial expansions"
    );
}
