#[test]
fn json_parser_is_a_checked_library_without_new_language_primitives() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/component-fixtures/json-value.wlt");
    let loaded = wlt::source::load(&path).unwrap();
    wlt::check(loaded.text()).unwrap();
    wlt::component::compile(loaded.text()).unwrap();
    wlt::component::core_js::compile(loaded.text()).unwrap();
}
