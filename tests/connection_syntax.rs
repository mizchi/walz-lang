use std::process::Command;

const CONNECTED: &str = r#"(component
  (import "host" (func $host (param "value" s32) (result s32)))
  (@lib (import "service" "call" (func $call (param $v i32) (result i32)))
    (func $run (param $v i32) (result i32) (@effect host) (call $call $v)))
  (@connect "service" "call" $host) (@expose "run" $run))"#;

#[test]
fn connect_supplies_imports_and_old_component_bind_is_diagnosed() {
    let built = walz::composition::compile(CONNECTED, &Default::default()).unwrap();
    assert!(built.wit.contains("run: func(v: s32) -> s32"));
    assert_eq!(built.manifest["connections"][0]["module"], "service");
    let error = walz::compile(&CONNECTED.replace("@connect", "@bind"))
        .err()
        .expect("retired annotation cannot disappear as opaque WAT");
    assert!(error.to_string().contains("@connect"), "{error}");
    let error = walz::compile("(module (@bind \"service\" \"call\" $f))")
        .err()
        .unwrap();
    assert!(error.to_string().contains("@connect"), "{error}");
    let error = walz::compile("(module (@component.binary \"walz:component/old\" \"\"))")
        .err()
        .unwrap();
    assert_eq!(error.diagnostic().code, "E_WASM_USE");
    let opaque = r#"(component (@note (@bind "data" "only")))"#;
    assert_eq!(
        walz::compile(opaque).unwrap().wasm,
        wat::parse_str(opaque).unwrap()
    );
}

#[test]
fn wit_bindgen_is_a_generator_command_and_bind_is_not_an_alias() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("empty.wit"),
        "package test:empty; world app {}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("policy.json"), r#"{"imports":[]}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(dir.path())
        .args(["wit-bindgen", "empty.wit", "policy.json", "generated"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in [
        "bindings.walz",
        "bindings.json",
        "types.walz",
        "exports.walz",
    ] {
        assert!(dir.path().join("generated").join(name).is_file());
    }
    let help = Command::new(env!("CARGO_BIN_EXE_walz"))
        .args(["wit-bindgen", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("wit-bindgen"));
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_walz"))
            .args(["bind", "--help"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn wasm_function_selection_is_rejected_even_when_unused() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("library.wasm"),
        wat::parse_str(
            r#"(component
      (core module $M (func (export "answer") (result i32) i32.const 42))
      (core instance $m (instantiate $M))
      (func $answer (result s32) (canon lift (core func $m "answer")))
      (export "answer" (func $answer)))"#,
        )
        .unwrap(),
    )
    .unwrap();
    for root in ["module", "component"] {
        let entry = dir.path().join("main.walz");
        std::fs::write(
            &entry,
            format!(
                r#"({root}
          (@use "./library.wasm" (func $answer)))"#
            ),
        )
        .unwrap();
        let error = walz::source::load(&entry)
            .err()
            .expect("@use cannot implicitly create a binary instance");
        assert_eq!(error.diagnostic().code, "E_WASM_USE", "{error}");
        assert!(error.to_string().contains("instantiate"), "{error}");
        assert!(error.to_string().contains("main.walz:2:"), "{error}");
    }
}

#[test]
fn selecting_a_constructor_does_not_instantiate_it_and_explicit_instances_are_retained() {
    let dir = tempfile::tempdir().unwrap();
    let binary=wat::parse_str("(component (core module $M (func $start unreachable) (start $start)) (core instance (instantiate $M)))").unwrap();
    std::fs::write(dir.path().join("trap.wasm"), &binary).unwrap();
    for (extra, expected) in [("", 0), ("(instance $unused (instantiate $Trap))", 1)] {
        let entry = dir.path().join("main.walz");
        std::fs::write(
            &entry,
            format!(r#"(component (@use "./trap.wasm" (component $Trap)) {extra})"#),
        )
        .unwrap();
        let loaded = walz::source::load(&entry).unwrap();
        let built = walz::compile(loaded.text()).unwrap();
        assert!(built.wasm.windows(binary.len()).any(|w| w == binary));
        let mut depth = 0;
        let mut count = 0;
        for p in wasmparser::Parser::new(0).parse_all(&built.wasm) {
            match p.unwrap() {
                wasmparser::Payload::Version { .. } => depth += 1,
                wasmparser::Payload::End(_) => depth -= 1,
                wasmparser::Payload::ComponentInstanceSection(s) if depth == 1 => {
                    count += s.count()
                }
                _ => {}
            }
        }
        assert_eq!(count, expected);
    }
}

#[test]
fn unresolved_constructor_files_require_a_source_loader() {
    let error = walz::compile(
        r#"(component
      (@use "./math.wasm" (component $Math))
      (instance $math (instantiate $Math)))"#,
    )
    .err()
    .unwrap();
    assert_eq!(error.diagnostic().code, "E_SOURCE_REQUIRED", "{error}");
}
