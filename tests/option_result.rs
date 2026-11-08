const SOURCE: &str = include_str!("component-fixtures/option-result.wlt");
#[test]
fn core_combinators_preserve_types_conditional_callbacks_and_effect_contracts() {
    wlt::check(SOURCE).unwrap();
    wlt::component::compile(SOURCE).unwrap();
    wlt::component::core_js::compile(SOURCE).unwrap();
    assert!(
        wlt::check(&SOURCE.replace("(%effect reads writes allocates)", "(%effect pure)")).is_err()
    );
    let source = r#"(module (%use "wlt:core" (type $option) (type $result) (type $future) (func $option.ok-or))
      (func $f (param $v (%inst $option (%inst $future i32))) (result (%inst $result (%inst $future i32) i32))
        (call (%inst $option.ok-or (%inst $future i32) i32) (%move $v) 1)))"#;
    assert!(
        wlt::check(source).is_err(),
        "owned payloads cannot bypass affine checking"
    );
}
