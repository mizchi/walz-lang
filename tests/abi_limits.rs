fn repeated(depth: usize) -> String {
    let mut s = "(module (type $T0 (struct (field $value i64))) (@abi.type $T0 (record \"value\"))"
        .to_owned();
    for n in 1..=depth {
        s += &format!(
            "(type $T{n} (struct (field $left (ref $T{})) (field $right (ref $T{})))) (@abi.type $T{n} (record \"left\" \"right\"))",
            n - 1,
            n - 1
        );
    }
    s + ")"
}
#[test]
fn inline_abi_work_is_bounded_before_flattening_or_arithmetic_overflow() {
    for depth in [12, 28, 40] {
        let error =
            walz::check(&repeated(depth)).expect_err("oversized inline ABI must be rejected");
        assert_eq!(error.diagnostic().code, "E_ABI_LIMIT", "{error}");
    }
}
#[test]
fn small_shared_dags_and_lists_keep_their_existing_contract() {
    let source = repeated(5);
    let m = walz::check(&source).unwrap();
    assert_eq!(m.types.len(), 6);
    let source = "(module (@use \"walz:core\" (type $list)) (type $R (struct (field $n i32))) (@abi.type $R (record \"n\")) (func $echo (export \"echo\") (param $v (@inst $list (ref $R))) (result (@inst $list (ref $R))) (@effect pure) $v))";
    walz::component::compile(source).unwrap();
    walz::component::core_js::compile(source).unwrap();
}

#[test]
fn adapter_parameter_limits_do_not_restrict_standard_core_wat() {
    let types = repeated(10);
    let source = format!(
        "{} (func $f (export \"f\") (param $a (ref $T10)) (param $b (ref $T10)) (result i32) (@effect pure) (i32.const 42)))",
        types.strip_suffix(')').unwrap()
    );
    walz::compile(&source).unwrap();
    assert_eq!(
        walz::component::compile(&source)
            .err()
            .unwrap()
            .diagnostic()
            .code,
        "E_ABI_LIMIT"
    );
}
