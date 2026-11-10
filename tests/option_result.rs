const SOURCE: &str = include_str!("component-fixtures/option-result.walz");
#[test]
fn core_combinators_preserve_types_conditional_callbacks_and_effect_contracts() {
    walz::check(SOURCE).unwrap();
    walz::component::compile(SOURCE).unwrap();
    walz::component::core_js::compile(SOURCE).unwrap();
    assert!(
        walz::check(&SOURCE.replace("(@effect reads writes allocates)", "(@effect pure)")).is_err()
    );
    let source = r#"(module (@use "walz:core" (type $option) (type $result) (type $future) (func $option.ok-or))
      (func $f (param $v (@inst $option (@inst $future i32))) (result (@inst $result (@inst $future i32) i32))
        (call (@inst $option.ok-or (@inst $future i32) i32) (@move $v) 1)))"#;
    assert!(
        walz::check(source).is_err(),
        "owned payloads cannot bypass affine checking"
    );
}
