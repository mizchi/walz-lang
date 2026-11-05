use wlt::bindings::BindingPolicy;

#[test]
fn generated_imports_require_separate_trusted_policy() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("host.wit");
    std::fs::write(
        &input,
        r#"package example:host@1.0.0;
      interface service {
        echo: func(value: string) -> result<tuple<s64, option<string>>, s32>;
        later: func(value: s64) -> future<result<s64, s32>>;
      }
      world guest { import service; }"#,
    )
    .unwrap();
    let grants = r#"{"imports":[
      {"module":"example:host/service@1.0.0","name":"echo","suspends":false,"capability":"echo"},
      {"module":"example:host/service@1.0.0","name":"later","suspends":false,"capability":"clock"}
    ]}"#;
    let generated = wlt::wit_bindgen::generate(&input, None, grants).unwrap();
    assert!(generated.source.contains("(pair i64 (option text))"));
    let source = generated.source.replacen("(module", "(module (func $main (export \"main\") (result (result (pair i64 (option text)) i32)) (effects host) (call $service.echo (text.lit \"日本語\")))", 1);
    assert!(wlt::component::compile(&source).is_err());
    let policy = BindingPolicy::parse(&generated.policy).unwrap();
    let module = wlt::check_with_policy(&source, &policy).unwrap();
    assert!(
        wlt::effects::infer(&module)
            .iter()
            .any(|e| e.host && !e.suspends)
    );
    wlt::component::compile_with_policy(&source, &policy).unwrap();
    wlt::component::core_js::compile_with_policy(&source, &policy).unwrap();
    let mut wrong = policy.clone();
    wrong.imports[0].binding.params[0] = "bytes".into();
    assert!(
        wlt::check_with_policy(&source, &wrong)
            .unwrap_err()
            .to_string()
            .contains("binding signature")
    );
}

#[test]
fn wit_generation_rejects_missing_authority_and_unsupported_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("host.wit");
    std::fs::write(
        &input,
        "package example:host; interface io { read: func() -> u32; } world guest { import io; }",
    )
    .unwrap();
    assert!(
        wlt::wit_bindgen::generate(&input, None, r#"{"imports":[]}"#)
            .unwrap_err()
            .to_string()
            .contains("policy")
    );
    let grants = r#"{"imports":[{"module":"example:host/io","name":"read","suspends":false,"capability":"input"}]}"#;
    assert!(
        wlt::wit_bindgen::generate(&input, None, grants)
            .unwrap_err()
            .to_string()
            .contains("unsupported WIT")
    );
}
