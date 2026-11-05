mod support;

#[test]
fn seeded_programs_agree_across_macro_expansion_optimization_and_engines() {
    support::fuzz::run();
}

#[test]
fn reducer_preserves_the_failure_and_reaches_a_structural_minimum() {
    use support::fuzz::Expr;
    let input = Expr::Add(
        Box::new(Expr::Const(91)),
        Box::new(Expr::Add(
            Box::new(Expr::Trace(42)),
            Box::new(Expr::Const(12)),
        )),
    );
    let reduced = support::fuzz::reduce(input, |e| e.has_trace());
    assert_eq!(reduced, Expr::Trace(42));
}

#[test]
fn reducer_can_shrink_a_payload_while_preserving_its_required_wrapper() {
    use support::fuzz::Expr;
    let input = Expr::Result(
        Box::new(Expr::Add(
            Box::new(Expr::Const(91)),
            Box::new(Expr::Trace(42)),
        )),
        true,
    );
    let reduced = support::fuzz::reduce(
        input,
        |e| matches!(e,Expr::Result(value,true) if value.has_trace()),
    );
    assert_eq!(reduced, Expr::Result(Box::new(Expr::Trace(42)), true));
}

#[test]
fn seeded_module_graphs_share_types_without_leaking_private_aliases() {
    for seed in 0..32 {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(
            dir.path().join("leaf.wlt"),
            format!(
                r#"(module
          (%export (opaque $State) (func $new) (func $get))
          (type $State (struct (field $n i32)))
          (func $new (result (ref $State)) (struct.new $State (i32.const {seed})))
          (func $get (param $s (ref $State)) (result i32) (struct.get $State $n (local.get $s))))"#
            ),
        )
        .unwrap();
        let private = format!("$hidden{seed}.read");
        std::fs::write(
            dir.path().join("facade.wlt"),
            format!(
                r#"(module
          (%import "sub/../leaf.wlt" (type $State $S) (func $new $make) (func $get {private}))
          (%export (type $S) (func $make)))"#
            ),
        )
        .unwrap();
        let main = dir.path().join("main.wlt");
        std::fs::write(
            &main,
            r#"(module
          (%import "facade.wlt" (type $S) (func $make))
          (%import "./leaf.wlt" (func $get $read))
          (func $main (export "main") (result i32) (call $read (call $make))))"#,
        )
        .unwrap();
        let loaded = wlt::source::load(&main).unwrap();
        let compiled = wlt::compile(loaded.text()).unwrap();
        assert_eq!(
            compiled
                .ir
                .unwrap()
                .types
                .iter()
                .filter(|t| t.name.ends_with(".State"))
                .count(),
            1
        );
        std::fs::write(
            &main,
            format!("(module (%import \"facade.wlt\" (func {private} $leaked)))"),
        )
        .unwrap();
        let loaded = wlt::source::load(&main).unwrap();
        assert!(wlt::check(loaded.text()).is_err(), "seed {seed}: {private}");
    }
}
