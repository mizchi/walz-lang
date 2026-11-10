fn module(body: &str) -> String {
    format!(
        r#"(module (@race-free)
      (import "env" "memory" (memory $m 1 1 shared))
      (@atomic.cell $cancel $m i32 0)
      (@readonly.region $data $m 16 64)
      (func $read (export "read") (param $offset i32) (result i32) (@effect reads)
        {body}))"#
    )
}

#[test]
fn bounded_readonly_loads_extend_the_atomic_certificate_without_raw_access() {
    let source = module("(@readonly.load $data i32 $offset)");
    let checked = walz::check(&source).unwrap();
    let cert = walz::race::certificate(&checked).unwrap().unwrap();
    assert_eq!(cert.version, 2);
    walz::race::verify_certificate(&cert).unwrap();
    walz::compile(&source).unwrap();
    let output = walz::component::core_js::compile(&source).unwrap();
    assert_eq!(output.manifest["abi"], "walz-core-js-v4");
    assert_eq!(
        output.manifest["raceFree"]["hostContract"],
        "walz-readonly-regions-v1"
    );
}

#[test]
fn readonly_regions_reject_overlap_overflow_and_forged_or_mutating_access() {
    for source in [
        module("(@readonly.load $data i32 $offset)").replace("16 64)", "0 64)"),
        module("(@readonly.load $data i32 $offset)").replace("16 64)", "65520 64)"),
        module("(@readonly.load $data i32 $offset)").replace("16 64)", "16 0)"),
        module("(@readonly.load $data i32 $offset)").replace("16 64)", "4294967295 4)"),
        module("(i32.load $m $offset)"),
        module("(@readonly.load $data i32.store $offset)"),
        module("(@readonly.load $missing i32 $offset)"),
    ] {
        assert!(walz::check(&source).is_err(), "{source}");
    }
}

#[test]
fn independent_kernel_rejects_readonly_certificate_tampering_and_checks_unused_layouts() {
    let checked = walz::check(&module("(@readonly.load $data i32 $offset)")).unwrap();
    let good = walz::race::certificate(&checked).unwrap().unwrap();
    for variant in 0..6 {
        let mut bad = good.clone();
        match variant {
            0 => bad.version = 1,
            1 => bad.readonly_accesses[0].opcode = "i32.store".into(),
            2 => bad.readonly_accesses[0].width = 8,
            3 => bad.readonly_accesses[0].region = 100,
            4 => bad.readonly_regions[0].offset = 0,
            _ => bad.readonly_regions[0].length = 65536,
        }
        assert!(
            walz::race::verify_certificate(&bad).is_err(),
            "variant {variant}"
        );
    }
    let mut unused = good;
    unused.readonly_accesses.clear();
    walz::race::verify_certificate(&unused).unwrap();
}
