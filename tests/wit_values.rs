#[test]
fn annotated_wit_values_compile_with_both_abis_and_keep_checked_storage() {
    let source = include_str!("component-fixtures/wit-values.walz");
    let module = walz::check(source).unwrap();
    let wit = walz::wit::emit(&module).unwrap();
    for word in ["bool", "char", "s8", "s16", "variant", "tuple<"] {
        assert!(wit.contains(word), "{wit}");
    }
    walz::component::compile(source).unwrap();
    walz::component::core_js::compile(source).unwrap();
    for bad in [
        "(module (type $T (@newtype i64)) (@abi.type $T (bool)))",
        "(module (type $T (@variant (x i32 i32))) (@abi.type $T (variant \"x\")))",
        "(module (type $T (@variant (x (ref $T)))) (@abi.type $T (variant \"x\")))",
    ] {
        assert!(walz::check(bad).is_err(), "{bad}");
    }
}

#[test]
fn wit_bindings_generate_all_shared_value_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shapes.wit");
    std::fs::write(&path,"package test:values; interface api { variant value { empty, text(string), number(f64), items(list<s32>) } type triple = tuple<bool,s8,char>; echo: func(v: value, n: s16, t: triple) -> value; } world guest { import api; export api; }").unwrap();
    let generated=walz::wit_bindgen::generate(&path,None,r#"{"imports":[{"module":"test:values/api","name":"echo","suspends":false,"capability":"echo"}]}"#).unwrap();
    let policy = walz::bindings::BindingPolicy::parse(&generated.policy).unwrap();
    walz::check_with_policy(&generated.source, &policy).unwrap();
    assert!(generated.source.contains("(variant"));
    assert!(generated.source.contains("(tuple)"));
}
