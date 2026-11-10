const SOURCE: &str = include_str!("component-fixtures/interface-types.walz");

#[test]
fn explicit_portable_types_preserve_nominal_checking_and_both_abis() {
    let module = walz::check(SOURCE).unwrap();
    let wit = walz::wit::emit(&module).unwrap();
    assert!(
        wit.contains("u32")
            && wit.contains("u64")
            && wit.contains("record")
            && wit.contains("enum")
            && wit.contains("flags"),
        "{wit}"
    );
    walz::component::compile(SOURCE).unwrap();
    walz::component::core_js::compile(SOURCE).unwrap();
    let wrong = SOURCE.replace(
        "(@effect pure) $v)\n  (func $wide",
        "(@effect pure) 0)\n  (func $wide",
    );
    assert!(walz::check(&wrong).is_err());
}

#[test]
fn abi_annotations_reject_mutability_recursion_and_wrong_storage() {
    for source in [
        "(module (type $T (struct)) (@abi.type $T (record)))",
        "(module (type $T (struct (field $n (mut i32)))) (@abi.type $T (record \"n\")))",
        "(module (type $T (struct (field $next (ref $T)))) (@abi.type $T (record \"next\")))",
        "(module (type $T (@newtype f32)) (@abi.type $T (u32)))",
        "(module (type $T (@variant (a i32))) (@abi.type $T (enum \"a\")))",
    ] {
        assert!(walz::check(source).is_err(), "{source}");
    }
}

#[test]
fn flags_share_the_pinned_component_limit_of_32_names() {
    let names = (0..33).map(|i| format!(" \"f{i}\"")).collect::<String>();
    let source = format!("(module (type $Flags (@newtype i64)) (@abi.type $Flags (flags{names})))");
    let error = walz::check(&source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_ABI_TYPE");
}
