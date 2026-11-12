const SCAN: &str = r#"(module
  (@use "walz:core" (type $bytes) (type $bytes-view)
    (func $bytes-view.new) (func $bytes-view.slice) (func $bytes-view.len)
    (func $bytes-view.read-i32-le))
  (func $word (param $v (ref $bytes-view)) (param $i i32) (result i32) (@effect pure)
    (call $bytes-view.read-i32-le $v $i))
  (func $sum (param $v (ref $bytes-view)) (result i64) (@effect pure)
    (@let $n (call $bytes-view.len $v)
      (@loop $scan (bind $i i32 0) (bind $sum i64 0) (result i64)
        (if (result i64) (i32.lt_u $i $n)
          (then (if (result i64) (i32.le_u 4 (- $n $i))
            (then (@recur $scan (+ $i 4) (+ $sum (i64.extend_i32_s (call $word $v $i)))))
            (else $sum)))
          (else $sum)))))
  (func $scan (export "scan") (param $b $bytes) (param $p i32) (param $n i32)
    (result i64) (@effect allocates)
    (call $sum (call $bytes-view.slice (call $bytes-view.new $b) $p $n))))"#;

#[test]
fn validated_view_ranges_reach_reads_through_calls_and_loop_guards() {
    let original =
        walz::compile_with_options(SCAN, walz::CompileOptions { optimize: false }).unwrap();
    let optimized = walz::compile(SCAN).unwrap();
    assert!(
        optimized.wat.contains("$__bytes_read_i32_le_proven"),
        "{}",
        optimized.wat
    );
    assert!(!optimized.wat.contains("bytes-view.check-width"));
    let helper = optimized
        .wat
        .split("(func $__bytes_read_i32_le_proven ")
        .nth(1)
        .unwrap()
        .split("(func ")
        .next()
        .unwrap();
    assert!(helper.contains("array.get_u"));
    assert!(!helper.contains("(if "));
    // The original checked source and diagnostics remain available for inspection.
    assert_eq!(
        original.ir.unwrap().functions.len(),
        optimized.ir.unwrap().functions.len()
    );
}

#[test]
fn signed_guards_wrapped_sums_mutable_reload_and_loop_initial_values_are_not_proofs() {
    for (extra, body) in [
        (
            "",
            "(if (result i32) (i32.lt_s $p (@bytes.len $b)) (then (@bytes.read-i32-le $b $p)) (else 0))",
        ),
        (
            "",
            "(if (result i32) (i32.le_u (+ $p 4) (@bytes.len $b)) (then (@bytes.read-i32-le $b $p)) (else 0))",
        ),
        (
            "",
            "(@loop $go (bind $i i32 0) (result i32) (if (result i32) (i32.lt_u $i 2) (then (block (result i32) (drop (@bytes.read-i32-le $b $i)) (@recur $go (+ $i 1)))) (else 0)))",
        ),
        (
            "(type $Cell (struct (field $p (mut i32))))",
            "(@let $c (struct.new $Cell $p) (if (result i32) (i32.le_u $p (@bytes.len $b)) (then (block (result i32) (struct.set $Cell $p $c -1) (@bytes.read-i32-le $b (struct.get $Cell $p $c)))) (else 0)))",
        ),
        (
            "(tag $Skip (param i32))",
            "(block (result i32) (@try (do (if (i32.eqz $p) (then (throw $Skip 0)) (else (drop (@bytes.read-i32-le $b $p))))) (catch $Skip ($code) (nop))) (@bytes.read-i32-le $b $p))",
        ),
        (
            "",
            "(@let $owner (@box.new i32 1) (block (result i32) (@box.close (@move $owner)) (+ (@bytes.read-i32-le $b $p) (@bytes.read-i32-be $b $p))))",
        ),
    ] {
        let source = format!(
            "(module (@use \"walz:core\" (type $bytes)) {extra} (func $f (export \"f\") (param $b $bytes) (param $p i32) (result i32) {body}))"
        );
        let compiled = walz::compile(&source).unwrap();
        assert!(
            !compiled.wat.contains("_proven"),
            "unsound range proof: {body}"
        );
    }
    // Compiler-only operations have no source spelling.
    assert!(
        walz::check("(module (func $f (result i32) (@bytes.read-i32-le-proven (@bytes.new) 0)))")
            .is_err()
    );
    for body in [
        "(block (result i32) (drop (@bytes.read-i32-le $b $p)) (@bytes.read-i32-le $other $p))",
        "(block (result i32) (drop (@bytes.read-i32-le $b $p)) (@bytes.read-i32-le $b $q))",
        "(block (result i32) (if (i32.eqz $q) (then (drop (@bytes.read-i32-le $b $p))) (else (nop))) (@bytes.read-i32-le $b $p))",
    ] {
        let source = format!(
            "(module (@use \"walz:core\" (type $bytes)) (func $f (export \"f\") (param $b $bytes) (param $other $bytes) (param $p i32) (param $q i32) (result i32) {body}))"
        );
        assert!(
            !walz::compile(&source).unwrap().wat.contains("_proven"),
            "{body}"
        );
    }
}

#[test]
fn optimized_and_original_wasm_agree_on_traps_side_effects_and_raw_open_world_inputs() {
    let scan = include_str!("component-fixtures/range-optimization.walz");
    let prefix = scan.trim_end().strip_suffix(')').unwrap();
    let source = format!(
        r#"{prefix}
      (import "env" "event" (func $event (result i32)))
      (type $A (array (mut i8)))
      (type $Nullable (array i32))
      (type $Cell (struct (field $p (mut i32))))
      (tag $Skip (param i32))
      (func $make (export "make") (param $n i32) (result $bytes)
        (@let $a (array.new $A 0 $n)
          (block (result $bytes)
            (@for $fill (bind $i i32 0) (while (i32.lt_u $i $n)) (step (+ $i 1))
              (array.set $A $a $i (+ (* $i 31) 7)))
            (@bytes.from-array $A $a))))
      (func $signed (export "signed") (param $b $bytes) (param $p i32) (result i32)
        (if (result i32) (i32.lt_s $p (@bytes.len $b)) (then (@bytes.read-i32-le $b $p)) (else 0)))
      (func $wrapped (export "wrapped") (param $b $bytes) (param $p i32) (result i32)
        (if (result i32) (i32.le_u (+ $p 4) (@bytes.len $b)) (then (@bytes.read-i32-le $b $p)) (else 0)))
      (func $caught (export "caught") (param $b $bytes) (param $p i32) (result i32)
        (block (result i32)
          (@try (do (if (i32.eqz $p) (then (throw $Skip 0))
            (else (drop (@bytes.read-i32-le $b $p))))) (catch $Skip ($code) (nop)))
          (@bytes.read-i32-le $b $p)))
      (func $mutable (export "mutable") (param $b $bytes) (result i32)
        (@let $c (struct.new $Cell 0)
          (if (result i32) (i32.le_u 4 (- (@bytes.len $b) (struct.get $Cell $p $c)))
            (then (block (result i32) (struct.set $Cell $p $c -1) (@bytes.read-i32-le $b (struct.get $Cell $p $c))))
            (else 0))))
      (func $loop (export "loop") (param $b $bytes) (result i32)
        (@loop $go (bind $i i32 0) (result i32)
          (if (result i32) (i32.lt_u $i 2)
            (then (block (result i32) (drop (@bytes.read-i32-le $b $i)) (@recur $go (+ $i 1))))
            (else 42))))
      (func $null (export "null") (param $a (ref null $Nullable)) (result i32)
        (if (result i32) (i32.ge_u (array.len $a) 0) (then 42) (else 0)))
      (func $condition (export "condition") (result i32)
        (if (result i32) (i32.ge_u (call $event) 0) (then 42) (else 0)))
      (func $argument (export "argument") (param $b $bytes) (result i32)
        (call $twice $b (i32.and (call $event) 0))))"#
    );
    let directory = tempfile::tempdir().unwrap();
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(&source, walz::CompileOptions { optimize }).unwrap();
        std::fs::write(
            directory
                .path()
                .join(if optimize { "on.wasm" } else { "off.wasm" }),
            compiled.wasm,
        )
        .unwrap();
    }
    let output=std::process::Command::new("node").args(["--input-type=module","-e",r#"
      import assert from 'node:assert/strict';
      import {readFile} from 'node:fs/promises';
      let calls=0, fail=false;
      const env={event:()=>{calls++;if(fail)throw new Error('event failed');return -1;}};
      const [off,on]=await Promise.all(['off','on'].map(async name=>(await WebAssembly.instantiate(await readFile(process.argv[1]+'/'+name+'.wasm'),{env})).instance.exports));
      const outcome=fn=>{try{return ['value',String(fn())];}catch(e){return ['error',e.constructor.name,e.message.includes('event failed')?'event failed':'wasm trap'];}};
      for(let n=0;n<70;n++){
        const a=off.make(n),b=on.make(n);
        for(const p of [-2147483648,-4,-1,0,1,n>>1,n,n+1,2147483647]){
          for(const method of ['signed','wrapped','twice','caught'])assert.deepEqual(outcome(()=>on[method](b,p)),outcome(()=>off[method](a,p)),method+' '+n+','+p);
          for(const length of [-1,0,1,3,4,5,n])for(const method of ['scan','repeat'])assert.deepEqual(outcome(()=>on[method](b,p,length)),outcome(()=>off[method](a,p,length)),`${method} ${n},${p},${length}`);
        }
        for(const method of ['mutable','loop'])assert.deepEqual(outcome(()=>on[method](b)),outcome(()=>off[method](a)),method);
      }
      for(const api of [off,on]){
        assert.throws(()=>api.null(null),WebAssembly.RuntimeError,'discarded condition must retain its null trap');
        calls=0;fail=false;assert.equal(api.condition(),42);assert.equal(calls,1);
        calls=0;fail=true;assert.throws(()=>api.condition(),/event failed/);assert.equal(calls,1);
        const bytes=api.make(4);
        calls=0;fail=false;assert.equal(api.argument(bytes),api.twice(bytes,0));assert.equal(calls,1,'inlined parameter evaluated once');
        calls=0;fail=true;assert.throws(()=>api.argument(bytes),/event failed/);assert.equal(calls,1);
      }
    "#]).arg(directory.path()).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_successful_read_proves_the_same_range_for_a_later_read() {
    let source = r#"(module (@use "walz:core" (type $bytes))
      (func $twice (export "twice") (param $b $bytes) (param $p i32) (result i32) (@effect pure)
        (+ (@bytes.read-i32-le $b $p) (@bytes.read-i32-be $b $p))))"#;
    let compiled = walz::compile(source).unwrap();
    assert!(
        compiled.wat.contains("$__bytes_read_i32_be_proven"),
        "{}",
        compiled.wat
    );
    assert!(compiled.wat.contains("(call $__bytes_read_i32_le "));
    for (first, proven) in [("i16", false), ("i64", true)] {
        let source = format!(
            "(module (@use \"walz:core\" (type $bytes)) (func $f (export \"f\") (param $b $bytes) (param $p i32) (result i32) (block (result i32) (drop (@bytes.read-{first}-le $b $p)) (@bytes.read-i32-be $b $p))))"
        );
        assert_eq!(
            walz::compile(&source)
                .unwrap()
                .wat
                .contains("$__bytes_read_i32_be_proven"),
            proven,
            "a successful {first} read must justify the entire next width"
        );
    }
}
