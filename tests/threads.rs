use wlt::{CompileOptions, Error};

fn opcodes() -> Vec<(String, Vec<&'static str>, Option<&'static str>)> {
    let mut ops = vec![
        ("atomic.fence".into(), vec![], None),
        (
            "memory.atomic.notify".into(),
            vec!["i32", "i32"],
            Some("i32"),
        ),
        (
            "memory.atomic.wait32".into(),
            vec!["i32", "i32", "i64"],
            Some("i32"),
        ),
        (
            "memory.atomic.wait64".into(),
            vec!["i32", "i64", "i64"],
            Some("i32"),
        ),
    ];
    for (ty, widths) in [("i32", vec![8, 16, 32]), ("i64", vec![8, 16, 32, 64])] {
        for width in widths {
            let full = width.to_string() == ty[1..];
            let load = if full {
                "load".into()
            } else {
                format!("load{width}_u")
            };
            let store = if full {
                "store".into()
            } else {
                format!("store{width}")
            };
            ops.push((format!("{ty}.atomic.{load}"), vec!["i32"], Some(ty)));
            ops.push((format!("{ty}.atomic.{store}"), vec!["i32", ty], None));
            for operation in ["add", "sub", "and", "or", "xor", "xchg", "cmpxchg"] {
                let suffix = if full {
                    format!("rmw.{operation}")
                } else {
                    format!("rmw{width}.{operation}_u")
                };
                let mut inputs = vec!["i32", ty];
                if operation == "cmpxchg" {
                    inputs.push(ty);
                }
                ops.push((format!("{ty}.atomic.{suffix}"), inputs, Some(ty)));
            }
        }
    }
    ops
}

fn error(source: &str, code: &str) -> Error {
    let error = wlt::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, code, "{source}: {error}");
    error
}

#[test]
fn every_threads_opcode_is_typed_and_emitted_in_both_optimization_modes() {
    let ops = opcodes();
    assert_eq!(ops.len(), 67);
    let mut source =
        String::from("(module (import \"env\" \"memory\" (memory $shared 1 2 shared))");
    for (i, (op, inputs, result)) in ops.iter().enumerate() {
        let memory = if op == "atomic.fence" {
            ""
        } else {
            " $shared offset=0"
        };
        let args = inputs
            .iter()
            .map(|ty| format!(" ({ty}.const 0)"))
            .collect::<String>();
        let result = result
            .map(|ty| format!(" (result {ty})"))
            .unwrap_or_default();
        source.push_str(&format!("(func $f{i} (export \"f{i}\"){result} (%effect reads writes synchronizes blocks) ({op}{memory}{args}))"));
    }
    source.push(')');
    let checked = wlt::check(&source).unwrap();
    assert_eq!(checked.functions.len(), 67);
    for optimize in [false, true] {
        let compiled = wlt::compile_with_options(&source, CompileOptions { optimize }).unwrap();
        assert!(compiled.ir.is_some());
        assert!(!compiled.analysis.opaque);
        for (opcode, _, _) in &ops {
            assert!(compiled.wat.contains(opcode), "{opcode}");
        }
    }
}

#[test]
fn shared_memory_types_effects_and_atomic_immediates_are_checked() {
    error("(module (memory $m 1 shared))", "E_MEMORY_DECL");
    error("(module (memory $m 2 1 shared))", "E_MEMORY_DECL");
    error(
        "(module (func $f (result i32) (i32.atomic.load 0)))",
        "E_MEMORY_USE",
    );
    error(
        "(module (memory $m 1 1 shared) (func $f (result i32) (i32.atomic.load $m align=1 0)))",
        "E_ATOMIC",
    );
    error(
        "(module (memory $m 1 1 shared) (func $f (result i32) (i32.atomic.load $m offset=4294967296 0)))",
        "E_ATOMIC",
    );
    error(
        "(module (memory $m 1 1 shared) (func $f (result i32) (i32.atomic.load $m (i64.const 0))))",
        "E_TYPE_MISMATCH",
    );
    error(
        "(module (memory $m 1 1 shared) (func $f (%effect pure) (atomic.fence)))",
        "E_EFFECT_CONTRACT",
    );
    error(
        "(module (memory $m 1 1 shared) (func $f (result i32) (%effect reads) (i32.atomic.load $m 0)))",
        "E_EFFECT_CONTRACT",
    );
    // Wait is valid on an unshared memory in Core Wasm and traps at execution.
    wlt::check("(module (memory $m 1) (func $f (result i32) (%effect reads synchronizes blocks) (memory.atomic.wait32 $m 0 0 0)))").unwrap();
}

#[test]
fn atomics_preserve_affine_ownership_and_do_not_transport_gc_values() {
    let prefix = r#"(module (%use "wlt:core" (type $box)) (memory $m 1 1 shared)"#;
    let good = format!(
        "{prefix} (func $run (param $owner (%inst $box i32)) (result i32) (%effect reads writes synchronizes blocks) (block (result i32) (i32.atomic.store $m 0 (block (result i32) (%box.close (%move $owner)) 7)) (memory.atomic.wait32 $m 0 6 0))))"
    );
    let module = wlt::check(&good).unwrap();
    wlt::ownership::validate(&good, &module).unwrap();
    for body in [
        "(i32.atomic.store $m 0 (%move $owner))",
        "(i32.atomic.store $m (block (result i32) (%box.close (%move $owner)) 0) (block (result i32) (%box.close (%move $owner)) 1))",
        "(drop (memory.atomic.wait32 $m 0 0 0))",
        "(drop (i32.atomic.rmw.add $m (return) (block (result i32) (%box.close (%move $owner)) 1)))",
    ] {
        let source = format!("{prefix} (func $bad (param $owner (%inst $box i32)) {body}))");
        assert!(wlt::check(&source).is_err(), "{source}");
        assert!(
            wlt::compile(&source).is_err(),
            "opaque fallback must not bypass ownership: {source}"
        );
    }
    error(
        "(module (memory $m 1 1 shared) (type $T (struct)) (func $bad (param $x (ref $T)) (i32.atomic.store $m 0 $x)))",
        "E_TYPE_MISMATCH",
    );
}

#[test]
fn synchronization_and_blocking_propagate_without_becoming_jspi_suspension() {
    let source = "(module (memory $m 1 1 shared) (func $wait (result i32) (%effect reads synchronizes blocks) (memory.atomic.wait32 $m 0 0 0)) (func $caller (export \"caller\") (result i32) (%effect reads synchronizes blocks) (call $wait)))";
    let compiled = wlt::compile(source).unwrap();
    let caller = compiled
        .analysis
        .functions
        .iter()
        .find(|f| f.name == "$caller")
        .unwrap();
    assert!(caller.semantic_effects.blocks && caller.semantic_effects.synchronizes);
    assert!(!caller.semantic_effects.suspends);
    assert!(caller.effects.may_block && caller.effects.may_diverge);
    assert!(!caller.effects.may_suspend);
    error(
        &source.replace(
            "(%effect reads synchronizes blocks) (call $wait)",
            "(%effect reads synchronizes) (call $wait)",
        ),
        "E_EFFECT_CONTRACT",
    );
}

#[test]
fn imported_memory_has_a_core_host_boundary_and_cannot_be_erased_into_wit() {
    let source = r#"(module (import "env" "memory" (memory $shared 1 2 shared))
      (func $add (export "add") (param $n i32) (result i32)
        (%effect reads writes synchronizes) (i32.atomic.rmw.add $shared 0 $n)))"#;
    let module = wlt::check(source).unwrap();
    assert_eq!(
        wlt::wit::emit(&module).unwrap_err().diagnostic().code,
        "E_COMPONENT_MEMORY"
    );
    assert_eq!(
        wlt::component::compile(source)
            .err()
            .unwrap()
            .diagnostic()
            .code,
        "E_COMPONENT_MEMORY"
    );
    let core = wlt::component::core_js::compile(source).unwrap();
    assert_eq!(core.manifest["memories"][0]["shared"], true);
    let private = source.replace(
        "(import \"env\" \"memory\" (memory $shared 1 2 shared))",
        "(memory $shared 1 2 shared)",
    );
    wlt::component::compile(&private).unwrap();
}

#[test]
fn file_modules_resolve_memory_names_and_dce_removes_unused_memory_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("counter.wlt"),
        r#"(module
      (import "unused" "memory" (memory $unused 1 1 shared))
      (import "env" "memory" (memory $mem 1 2 shared))
      (%export (func $add) (memory $mem))
      (func $dead (result i32) (i32.atomic.load $unused 0))
      (func $add (param $n i32) (result i32) (%effect reads writes synchronizes)
        (i32.atomic.rmw.add $mem 0 $n)))"#,
    )
    .unwrap();
    let main = dir.path().join("main.wlt");
    std::fs::write(&main, r#"(module
      (%use "./counter.wlt" (func $add $plus) (memory $mem $shared))
      (export "shared" (memory $shared))
      (func $main (export "main") (result i32) (%effect reads writes synchronizes) (call $plus 1)))"#).unwrap();
    let source = wlt::source::load(main).unwrap();
    let compiled = wlt::compile(source.text()).unwrap();
    let module = compiled.ir.unwrap();
    // Inspection IR retains all checked declarations; only emitted imports are pruned.
    assert_eq!(module.memories.len(), 2);
    let memories = wasmparser::Parser::new(0)
        .parse_all(&compiled.wasm)
        .filter_map(|payload| {
            if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
                Some(section)
            } else {
                None
            }
        })
        .flat_map(|section| section.into_imports())
        .filter(|import| matches!(import.as_ref().unwrap().ty, wasmparser::TypeRef::Memory(_)))
        .count();
    assert_eq!(memories, 1);
    assert!(!compiled.wat.contains("unused"));
    assert!(compiled.wat.contains("(export \"shared\" (memory"));
}

#[test]
fn native_threads_wat_stays_unchanged() {
    let source = r#"(module (import "env" "memory" (memory 1 1 shared))
      (func (export "add") (param i32) (result i32)
        i32.const 0 local.get 0 i32.atomic.rmw.add))"#;
    assert_eq!(wlt::compile(source).unwrap().wat, source);
}

#[test]
fn fences_without_memory_and_unshared_atomic_wait_follow_core_validation() {
    for source in [
        "(module (func $f (export \"f\") (%effect synchronizes) (atomic.fence)))",
        "(module (memory $m 1) (func $f (export \"f\") (result i32) (%effect reads synchronizes blocks) (memory.atomic.wait32 $m 0 0 0)))",
    ] {
        assert!(wlt::compile(source).unwrap().ir.is_some());
    }
}

#[test]
fn memory_indices_limits_and_reserved_abi_capability_are_checked() {
    let source = r#"(module
      (memory $local (export "local") 1 1 shared)
      (import "env" "memory" (memory $foreign 1 2 shared))
      (func $read (export "read") (result i32) (%effect reads synchronizes)
        (+ (i32.atomic.load 0 0) (i32.atomic.load 1 offset=4 align=4 0))))"#;
    let module = wlt::check(source).unwrap();
    assert_eq!(module.memories[0].name, "$foreign");
    assert_eq!(module.memories[1].name, "$local");
    wlt::compile(source).unwrap();
    for source in [
        "(module (memory $m 65537))",
        "(module (memory $m 1 1 shared shared))",
        "(module (memory $m i64 1 1 shared))",
        "(module (memory $m 1) (memory $m 1))",
        "(module (memory $m (export \"f\") 1) (func $f (export \"f\") (nop)))",
    ] {
        assert!(wlt::check(source).is_err(), "{source}");
    }
    let source = r#"(module (import "__component" "memory" (memory $m 1 1 shared)) (func $f (export "f") (result i32) (%effect reads synchronizes) (i32.atomic.load $m 0)))"#;
    assert_eq!(
        wlt::component::core_js::compile(source)
            .err()
            .unwrap()
            .diagnostic()
            .code,
        "E_MEMORY_IMPORT"
    );
}
