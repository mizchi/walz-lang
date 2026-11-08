const SOURCE: &str = include_str!("component-fixtures/local-cancellation.wlt");
#[test]
fn local_cancellation_is_typed_affine_and_portable_to_both_js_backends() {
    wlt::check(SOURCE).unwrap();
    wlt::component::compile(SOURCE).unwrap();
    wlt::component::core_js::compile(SOURCE).unwrap();
    for source in [
        SOURCE.replace("%task.cancel i32 (%move $t)", "%task.cancel i64 (%move $t)"),
        SOURCE.replace("%task.cancel i32 (%move $t)", "%task.cancel i32 $t"),
        SOURCE.replace(
            "%task.join-timeout i32 5",
            "%task.join-timeout i32 (i64.const 5)",
        ),
        SOURCE.replace(
            "(%task.cancel i32 (%move $t))",
            "(block (%task.cancel i32 (%move $t)) (%task.drop i32 (%move $t)))",
        ),
    ] {
        assert!(
            wlt::check(&source).is_err(),
            "accepted invalid cancellation: {source}"
        );
    }
}
