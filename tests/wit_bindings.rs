use walz::bindings::BindingPolicy;

#[test]
fn wit_generation_rejects_flags_exceeding_the_component_limit() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("flags.wit");
    let names = (0..33)
        .map(|i| format!("flag{i}"))
        .collect::<Vec<_>>()
        .join(",");
    std::fs::write(&input,format!("package example:host@1.0.0; interface service {{ flags many {{ {names} }} echo: func(value: many) -> many; }} world guest {{ import service; }}")).unwrap();
    let grants = r#"{"imports":[{"module":"example:host/service@1.0.0","name":"echo","suspends":false,"capability":"flags"}]}"#;
    let error = walz::wit_bindgen::generate(&input, None, grants)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("cannot have more than 32 flags"),
        "{error}"
    );
}

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
    let generated = walz::wit_bindgen::generate(&input, None, grants).unwrap();
    assert!(
        generated
            .policy
            .contains("(result (pair i64 (option text)) i32)")
    );
    assert!(
        generated
            .source
            .contains("(@inst $tuple i64 (@inst $option $string))")
    );
    let source = generated.source.replacen("(module", "(module (func $main (export \"main\") (result (@result (@inst $tuple i64 (@inst $option $string)) i32)) (@effect host) (call $service.echo (@string.lit \"日本語\")))", 1);
    assert!(walz::component::compile(&source).is_err());
    let policy = BindingPolicy::parse(&generated.policy).unwrap();
    let module = walz::check_with_policy(&source, &policy).unwrap();
    assert!(
        walz::effects::infer(&module)
            .iter()
            .any(|e| e.host && !e.suspends)
    );
    walz::component::compile_with_policy(&source, &policy).unwrap();
    walz::component::core_js::compile_with_policy(&source, &policy).unwrap();
    let mut wrong = policy.clone();
    wrong.imports[0].binding.params[0] = "bytes".into();
    assert!(
        walz::check_with_policy(&source, &wrong)
            .unwrap_err()
            .to_string()
            .contains("binding signature")
    );
}

#[test]
fn wit_records_enums_flags_unsigned_and_export_scaffolds_share_checked_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("types.wit");
    std::fs::write(
        &path,
        r#"package example:shapes@1.0.0;
      interface io {
        enum color { red, green, blue }
        flags access { read, write }
        record item { label: string, count: u32, wide: u64, color: color, access: access }
        echo: func(value: item) -> item;
      }
      world guest { import io; export io; }
      world empty {}"#,
    )
    .unwrap();
    let grants = r#"{"imports":[{"module":"example:shapes/io@1.0.0","name":"echo","suspends":false,"capability":"echo"}]}"#;
    let generated = walz::wit_bindgen::generate(&path, Some("guest"), grants).unwrap();
    assert!(generated.source.contains("@abi.type"));
    assert!(
        generated
            .source
            .contains("(record \"label\" \"count\" \"wide\" \"color\" \"access\")")
    );
    let policy = BindingPolicy::parse(&generated.policy).unwrap();
    let source=generated.source.replacen("(module",r#"(module (func $main (export "main") (param $v (ref $io.item)) (result (ref $io.item)) (@effect host) (call $io.echo $v))"#,1);
    walz::component::compile_with_policy(&source, &policy).unwrap();
    walz::component::core_js::compile_with_policy(&source, &policy).unwrap();
    assert!(walz::wit_bindgen::generate(&path, Some("empty"), grants).is_err());
    std::fs::write(dir.path().join("grants.json"), grants).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_walz"))
        .args([
            "wit-bindgen",
            path.to_str().unwrap(),
            dir.path().join("grants.json").to_str().unwrap(),
            dir.path().join("generated").to_str().unwrap(),
            "--world",
            "guest",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let export = std::fs::read_to_string(dir.path().join("generated/exports.walz")).unwrap();
    assert!(
        export.contains("unreachable") && export.contains("export \"io-echo\""),
        "{export}"
    );
    let loaded = walz::source::load(dir.path().join("generated/exports.walz")).unwrap();
    walz::component::core_js::compile(loaded.text()).unwrap();
}

#[test]
fn wit_generation_rejects_missing_authority_and_unsupported_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("host.wit");
    std::fs::write(
        &input,
        "package example:host; interface io { resource handle; read: func() -> list<own<handle>>; } world guest { import io; }",
    )
    .unwrap();
    assert!(
        walz::wit_bindgen::generate(&input, None, r#"{"imports":[]}"#)
            .unwrap_err()
            .to_string()
            .contains("policy")
    );
    let grants = r#"{"imports":[{"module":"example:host/io","name":"read","suspends":false,"capability":"input"}]}"#;
    assert!(
        walz::wit_bindgen::generate(&input, None, grants)
            .unwrap_err()
            .to_string()
            .contains("unsupported WIT")
    );
}

#[test]
fn generated_lists_resolve_aliases_and_reject_owned_elements_early() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("list.wit");
    let grants = r#"{"imports":[{"module":"example:lists/io","name":"read","suspends":false,"capability":"input"}]}"#;
    std::fs::write(&input,"package example:lists; interface io { type element = result<tuple<s64,string>,s32>; read: func() -> list<element>; } world guest { import io; }").unwrap();
    let generated = walz::wit_bindgen::generate(&input, None, grants).unwrap();
    assert!(
        generated
            .source
            .contains("(@inst $list (@inst $result (@inst $tuple i64 $string) i32))")
    );
    std::fs::write(&input,"package example:lists; interface io { read: func() -> list<option<future<s64>>>; } world guest { import io; }").unwrap();
    assert!(
        walz::wit_bindgen::generate(&input, None, grants)
            .unwrap_err()
            .to_string()
            .contains("shared data")
    );
}
