const SOURCE: &str = include_str!("component-fixtures/native-async.walz");
#[test]
fn native_async_world_uses_standard_internal_producers() {
    let d = tempfile::tempdir().unwrap();
    let wit = d.path().join("api.wit");
    std::fs::write(&wit, include_str!("component-fixtures/native-async.wit")).unwrap();
    let c = walz::component::compile_with_world(SOURCE, &Default::default(), &wit, Some("guest"))
        .unwrap();
    assert!(c.wat.contains("canon future.new"));
    assert!(c.wat.contains("canon stream.write"));
    assert!(!c.wat.contains("(import \"walz:runtime/"));
}

#[test]
#[ignore = "requires Wasmtime 50+ with stackful Component async; just test-native-engine"]
fn native_engine_runs_the_canonical_future_producer_without_javascript() {
    let dir = tempfile::tempdir().unwrap();
    let wit = dir.path().join("api.wit");
    std::fs::write(&wit, include_str!("component-fixtures/native-async.wit")).unwrap();
    let built =
        walz::component::compile_with_world(SOURCE, &Default::default(), &wit, Some("guest"))
            .unwrap();
    let body = built
        .wat
        .trim()
        .strip_prefix("(component")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let harness = format!(
        r#"(component
      (component $built {body}) (instance $built (instantiate $built))
      (alias export $built "test:native/api" (instance $api))
      (core module $memory (memory (export "memory") 1)) (core instance $memory (instantiate $memory))
      (type $f (future s32))
      (core func $ready (canon lower (func $api "ready")))
      (core func $read (canon future.read $f (memory (core memory $memory "memory"))))
      (core func $drop (canon future.drop-readable $f))
      (core module $run
        (import "" "memory" (memory 1)) (import "" "ready" (func $ready (result i32)))
        (import "" "read" (func $read (param i32 i32) (result i32))) (import "" "drop" (func $drop (param i32)))
        (func (export "run") (result i32) (local $f i32)
          (local.set $f (call $ready)) (drop (call $read (local.get $f) (i32.const 0)))
          (call $drop (local.get $f)) (i32.load (i32.const 0))))
      (core instance $run (instantiate $run (with "" (instance
        (export "memory" (memory $memory "memory")) (export "ready" (func $ready)) (export "read" (func $read)) (export "drop" (func $drop))))))
      (func (export "run") async (result s32) (canon lift (core func $run "run"))))"#
    );
    let wasm = wat::parse_str(&harness).unwrap();
    let file = dir.path().join("harness.wasm");
    std::fs::write(&file, wasm).unwrap();
    let output=std::process::Command::new(std::env::var_os("WALZ_WASMTIME").unwrap_or_else(||"wasmtime".into())).args(["run","-W","gc=y,function-references=y,exceptions=y,component-model-async=y,component-model-more-async-builtins=y,component-model-async-stackful=y","--invoke","run()"]).arg(file).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}
