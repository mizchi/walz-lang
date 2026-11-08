fn checked(body: &str) -> String {
    format!(
        r#"(module
      (%race-free)
      (import "env" "memory" (memory $m 1 1 shared))
      (%atomic.cell $counter $m i32 0)
      (%atomic.cell $wide $m i64 8)
      (func $add (export "add") (param $n i32) (result i32)
        (%effect reads writes synchronizes) {body}))"#
    )
}

#[test]
fn static_atomic_cells_have_a_machine_checked_conditional_race_contract() {
    let source = checked("(i32.atomic.rmw.add (%atomic.at $counter) $n)");
    let module = wlt::check(&source).unwrap();
    assert!(module.race_free);
    let certificate = wlt::race::certificate(&module).unwrap().unwrap();
    wlt::race::verify_certificate(&certificate).unwrap();
    wlt::verified::compile(&source).unwrap();
    let built = wlt::component::core_js::compile(&source).unwrap();
    assert_eq!(built.manifest["abi"], "wlt-core-js-v3");
    assert_eq!(built.manifest["raceFree"]["hostRequired"], true);
    assert_eq!(
        built.manifest["raceFree"]["hostContract"],
        "wlt-atomic-cells-v1"
    );
    assert_eq!(
        built.manifest["raceFree"]["cells"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn certified_memory_accesses_preserve_local_gc_ownership_checks() {
    let source = checked(
        "(%let $owner (%box.new i32 $n) (block (result i32) (%box.close (%move $owner)) (i32.atomic.rmw.add (%atomic.at $counter) $n)))",
    )
    .replace("(%race-free)", "(%race-free) (%use \"wlt:core\" (type $box))");
    wlt::verified::compile(&source).unwrap();
    for replacement in [
        "(nop)",
        "(block (%box.close (%move $owner)) (%box.close (%move $owner)))",
    ] {
        let bad = source.replace("(%box.close (%move $owner))", replacement);
        assert!(wlt::check(&bad).is_err(), "{bad}");
        assert!(wlt::verified::compile(&bad).is_err(), "{bad}");
    }
}

#[test]
fn raw_addresses_forgery_offsets_and_partial_aliases_cannot_get_a_certificate() {
    for body in [
        "(i32.atomic.load $m 0)",
        "(i32.atomic.load $m $n)",
        "(i32.atomic.load $m offset=4 (%atomic.at $counter))",
        "(i32.atomic.load16_u (%atomic.at $counter))",
        "(i64.atomic.load32_u (%atomic.at $counter))",
        "(i32.atomic.load (%atomic.at $wide))",
        "(i32.add (%atomic.at $counter) 0)",
        "(memory.grow $m 1)",
        "(i32.load $m (i32.const 0))",
    ] {
        let source = checked(body);
        assert!(wlt::check(&source).is_err(), "{body}");
        assert!(
            wlt::compile(&source).is_err(),
            "fallback bypassed verification: {body}"
        );
    }
    for declaration in [
        "(%atomic.cell $overlap $m i32 8)",
        "(%atomic.cell $unaligned $m i64 4)",
        "(%atomic.cell $outside $m i32 65536)",
    ] {
        let source = checked("(i32.atomic.rmw.add (%atomic.at $counter) $n)").replacen(
            "(func $add",
            &format!("{declaration} (func $add"),
            1,
        );
        assert!(wlt::check(&source).is_err(), "{declaration}");
    }
}

#[test]
fn kernel_rejects_tampered_layouts_and_non_atomic_or_wrong_width_footprints() {
    let module = wlt::check(&checked("(i32.atomic.rmw.add (%atomic.at $counter) $n)")).unwrap();
    let good = wlt::race::certificate(&module).unwrap().unwrap();
    let mut bad = good.clone();
    bad.cells[0].width = 8;
    bad.cells[1].offset = 4;
    bad.cells[1].width = 4;
    bad.accesses[0].opcode = "i64.atomic.load".into();
    bad.accesses[0].width = 8;
    assert_eq!(
        wlt::race::verify_certificate(&bad)
            .unwrap_err()
            .diagnostic()
            .code,
        "E_RACE_KERNEL"
    );
    for opcode in ["i32.store", "i64.atomic.load", "i32.atomic.load16_u"] {
        let mut bad = good.clone();
        bad.accesses[0].opcode = opcode.into();
        assert!(wlt::race::verify_certificate(&bad).is_err(), "{opcode}");
    }
    let mut bad = good;
    bad.accesses[0].offset = 4;
    assert!(wlt::race::verify_certificate(&bad).is_err());
}

#[test]
fn verification_has_no_host_callbacks_resizing_raw_exports_or_gc_boundary() {
    let good = checked("(i32.atomic.rmw.add (%atomic.at $counter) $n)");
    for source in [
        good.replace("1 1 shared", "1 2 shared"),
        good.replace(
            "(func $add",
            "(import \"host\" \"unused\" (func $host)) (func $add",
        ),
        good.replace("(func $add", "(export \"memory\" (memory $m)) (func $add"),
        good.replace("(func $add", "(memory $other 1 1 shared) (func $add"),
        good.replace("(param $n i32)", "(param $n (ref $T))")
            .replace("(func $add", "(type $T (struct)) (func $add")
            .replace("$n)", "0)"),
    ] {
        assert!(wlt::check(&source).is_err(), "{source}");
    }
    let dead = good.replace(
        "(func $add",
        "(func $dead (result i32) (i32.atomic.load $m 0)) (func $add",
    );
    assert_eq!(
        wlt::check(&dead).unwrap_err().diagnostic().code,
        "E_RACE_ACCESS"
    );
    let error = wlt::component::core_js::compile_for_browser(&good, wlt::CompileOptions::default())
        .err()
        .unwrap();
    assert_eq!(error.diagnostic().code, "E_HOST_UNAVAILABLE");
    let private = good.replace(
        "(import \"env\" \"memory\" (memory $m 1 1 shared))",
        "(memory $m 1 1 shared)",
    );
    wlt::component::compile(&private).unwrap();
    wlt::compile_with_options(&private, wlt::CompileOptions { optimize: false }).unwrap();
}

#[test]
fn static_cell_aliases_work_across_files_and_do_not_escape_as_integers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("state.wlt"),
        r#"(module
      (import "env" "memory" (memory $m 1 1 shared))
      (%atomic.cell $counter $m i32 0)
      (%export (cell $counter) (func $add))
      (func $add (param $n i32) (result i32) (%effect reads writes synchronizes)
        (i32.atomic.rmw.add (%atomic.at $counter) $n)))"#,
    )
    .unwrap();
    let path = dir.path().join("main.wlt");
    std::fs::write(
        &path,
        r#"(module (%race-free)
      (%use "./state.wlt" (cell $counter $cell) (func $add))
      (func $main (export "main") (result i32) (%effect reads writes synchronizes)
        (+ (call $add 1) (i32.atomic.load (%atomic.at $cell)))))"#,
    )
    .unwrap();
    let source = wlt::source::load(&path).unwrap();
    let compiled = wlt::compile(source.text()).unwrap();
    assert!(compiled.ir.unwrap().race_free);
    assert!(!compiled.wat.contains("atomic.at"));
    std::fs::write(&path, r#"(module (%use "./state.wlt" (cell $counter)) (func $forge (result i32) (%atomic.at $counter)))"#).unwrap();
    let source = wlt::source::load(&path).unwrap();
    assert!(wlt::check(source.text()).is_err());
}
