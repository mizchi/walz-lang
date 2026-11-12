const SOURCE: &str = r#"(module
  (@use "walz:core" (type $tuple) (type $result) (type $option))
  (type $Box (@type.params $T) (struct (field $value $T)))
  (func $id (@type.params $T) (@where (shared $T)) (param $x $T) (result $T) (@effect pure) $x)
  (func $wrap (param $n i32) (result $result<$tuple<i32, i32>, i32>) (@effect pure)
    (@result.ok (@tuple.new $n (i32.const 2))))
  (func $main (export "main") (result i32) (@effect allocates)
    (@let $box (struct.new $Box<i32> (call $id<i32> 40))
      (@match (call $wrap (struct.get $Box<i32> $value $box))
        (@case err ($code) $code)
        (@case ok ($pair) (@let ($a $b) $pair (+ $a $b)))))))"#;

#[test]
fn angle_types_heap_names_and_calls_lower_to_existing_instantiations() {
    let explicit = SOURCE
        .replace(
            "$result<$tuple<i32, i32>, i32>",
            "(@inst $result (@inst $tuple i32 i32) i32)",
        )
        .replace("$Box<i32>", "(@inst $Box i32)")
        .replace("$id<i32>", "(@inst $id i32)");
    for optimize in [false, true] {
        let options = walz::CompileOptions { optimize };
        assert_eq!(
            walz::compile_with_options(SOURCE, options).unwrap().wasm,
            walz::compile_with_options(&explicit, options).unwrap().wasm
        );
    }
    let spaced = SOURCE.replace(
        "$result<$tuple<i32, i32>, i32>",
        "$result<\n $tuple<i32, (; inner ;) i32>,\n i32,\n>",
    );
    assert_eq!(
        walz::compile(SOURCE).unwrap().wasm,
        walz::compile(&spaced).unwrap().wasm
    );
    let compact = SOURCE.replace(
        "(struct.new $Box<i32> (call $id<i32> 40))",
        "(@$Box<i32>::{:value (call $id<i32> 40)})",
    );
    let named = SOURCE.replace(
        "(struct.new $Box<i32> (call $id<i32> 40))",
        "(@struct.new $Box<i32> {:value (call $id<i32> 40)})",
    );
    assert_eq!(
        walz::compile(&named).unwrap().wasm,
        walz::compile(&compact).unwrap().wasm
    );
}

#[test]
fn literal_wat_names_and_operators_keep_their_meaning() {
    let quoted = r#"(module (func $main (export "main") (result i32)
      (@let $"x" (i32.const 42) (local.get $x))))"#;
    walz::compile(quoted).unwrap();
    let quoted = r#"(module (func $"id<i32>" (result i32) (i32.const 42))
      (func $main (export "main") (result i32) (@let $x (i32.const 1) (call $id<i32>))))"#;
    walz::compile(quoted).unwrap();
    let source = "(module (func $id<i32> (result i32) (i32.const 42)) (func $main (export \"main\") (result i32) (call $id<i32>)))";
    assert_eq!(walz::compile(source).unwrap().wat, source);
    let mixed = r#"(module
      (func $id (@type.params $T) (param $x $T) (result $T) $x)
      (func $id<i32> (result i32) (@effect pure) 42)
      (func $main (export "main") (result i32) (@effect pure)
        (@let $x<i32> (i32.const 42) (if (result i32) (< $x<i32> 43)
          (then (call $id<i32>)) (else (i32.shr_u $x<i32> 1))))))"#;
    let built = walz::compile(mixed).unwrap();
    assert!(built.wat.contains("(call $id<i32>)"));
    for name in ["$id<i32>>", "$id<i32>::", "$id<i32>.suffix"] {
        let source = format!(
            "(module (func $id (@type.params $T) (param $x $T) (result $T) $x) (func {name} (result i32) (@effect pure) 42) (func $main (export \"main\") (result i32) (@effect pure) (call {name})))"
        );
        assert!(
            walz::compile(&source)
                .unwrap()
                .wat
                .contains(&format!("(call {name})"))
        );
    }
}

#[test]
fn formatter_and_review_view_preserve_angle_applications() {
    let formatted = walz::format::source(SOURCE).unwrap();
    assert!(
        formatted.contains("$result<$tuple<i32, i32>, i32>"),
        "{formatted}"
    );
    assert_eq!(walz::format::source(&formatted).unwrap(), formatted);
    assert_eq!(
        walz::compile(&formatted).unwrap().wasm,
        walz::compile(SOURCE).unwrap().wasm
    );
    let review = walz::review::to_watz(SOURCE).unwrap();
    assert!(
        review.contains("$result<$tuple<i32, i32>, i32>"),
        "{review}"
    );
    let lowered = walz::review::from_watz(&review).unwrap();
    assert_eq!(
        walz::compile(&lowered.text).unwrap().wasm,
        walz::compile(SOURCE).unwrap().wasm
    );
}

#[test]
fn malformed_argument_lists_get_specific_diagnostics() {
    for (application, message) in [
        ("$tuple<>", "at least one"),
        ("$tuple<i32 i32>", "','"),
        ("$tuple<i32,, i32>", "type argument"),
        ("$tuple<i32", "'>'"),
    ] {
        let source = format!(
            "(module (@use \"walz:core\" (type $tuple)) (func $f (param $v {application}) (nop)))"
        );
        let error = walz::check(&source).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_GENERIC_SYNTAX", "{error}");
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn macro_type_arguments_and_declarative_exports_accept_the_same_sugar() {
    let source = r#"(module
      (@use "walz:core" (type $tuple))
      (@macro $as (rule (?T:type ?x:expr) (@let $v (type ?T) ?x $v)))
      (func $id (@type.params $T) (param $x $T) (result $T) $x)
      (export "identity" (func $id<i32>))
      (func $main (export "main") (result i32) (@effect pure)
        (@let ($a $b) (@$as $tuple<i32, i32> (@tuple.new (i32.const 40) (i32.const 2))) (+ $a $b))))"#;
    let explicit = source
        .replace("$id<i32>", "(@inst $id i32)")
        .replace("$tuple<i32, i32>", "(@inst $tuple i32 i32)");
    assert_eq!(
        walz::compile(source).unwrap().wasm,
        walz::compile(&explicit).unwrap().wasm
    );
}

#[test]
fn constraints_type_errors_and_original_spans_are_unchanged() {
    let source = "(module (func $add (@type.params $T) (@where (Add $T)) (param $x $T) (result $T) (+ $x $x)) (func $f (result i32) (call $add<i32> (i64.const 1))))";
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_TYPE_MISMATCH");
    let span = error.diagnostic().primary.as_ref().unwrap().span;
    assert_eq!(&source[span.start..span.end], "(i64.const 1)");
    let source = "(module (type $C (struct)) (func $id (@type.params $T) (@where (Add $T)) (param $x $T) (result $T) $x) (func $f (param $x (ref $C)) (result (ref $C)) (call $id<(ref $C)> $x)))";
    assert!(walz::check(source).unwrap_err().to_string().contains("Add"));
    let source =
        "(module (@use \"walz:core\" (type $option)) (func $f (param $x $option<$Missing>) (nop)))";
    let error = walz::check(source).unwrap_err();
    let span = error.diagnostic().primary.as_ref().unwrap().span;
    assert_eq!(&source[span.start..span.end], "$Missing");
}

#[test]
fn editor_resolves_applications_and_recovers_incomplete_argument_lists() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.walz");
    let uri = format!("file://{}", path.display());
    let mut session = walz::lsp::Session::new(Default::default());
    let source = "(module\n (func $id (@type.params $T) (param $x $T) (result $T) $x)\n (func $main (result i32) (; 🦀 ;) (call $id<i32> 42)))";
    let out = session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":source}}}));
    assert!(
        out.iter().any(|v| v["params"]["diagnostics"] == json!([])),
        "{out:?}"
    );
    let index = source.find("$id<i32>").unwrap();
    let prefix = &source[..index];
    let pos = json!({"line":prefix.bytes().filter(|b| *b == b'\n').count(), "character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()});
    let out = session.handle(json!({"id":1,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":pos}}));
    assert_eq!(out[0]["result"][0]["range"]["start"]["line"], 1, "{out:?}");
    let changed = source.replace("$id<i32> 42)))", "$id<i32,");
    let out = session.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":changed}]}}));
    assert!(
        out.iter().any(|v| v["params"]["diagnostics"]
            .as_array()
            .is_some_and(|v| !v.is_empty())),
        "{out:?}"
    );
    let out = session.handle(json!({"id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":pos}}));
    assert!(
        out[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["label"] == "$id"),
        "{out:?}"
    );
}

#[test]
fn nesting_comments_and_opaque_metadata_do_not_change_acceptance_or_panic() {
    let mut ty = "i32".to_owned();
    for _ in 0..260 {
        ty = format!("$option<{ty}>");
    }
    let source =
        format!("(module (@use \"walz:core\" (type $option)) (func $f (param $v {ty}) (nop)))");
    assert_eq!(
        walz::check(&source).unwrap_err().diagnostic().code,
        "E_GENERIC_SYNTAX"
    );
    for prefix in ["(@use)", "(@file.namespace)", "(@inst)"] {
        let source = format!("(module {prefix} (func $f (param $x $T<i32>) (nop)))");
        assert!(walz::check(&source).is_err());
    }
    let source = r#"(module (@metadata $NoSuch<,> (@use "./missing.walz"))
      (func $main (export "main") (result i32) (@effect pure) 42))"#;
    // Unknown metadata is opaque: neither the malformed angle data nor the
    // nested file use is interpreted. It remains outside the typed core.
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_CHECK");
    assert!(error.to_string().contains("unsupported declaration"));
    let quoted = r#"(module (func $"f<i32>" (result i32) (i32.const 42))
      (func (export "main") (result i32) (call $"f<i32>")))"#;
    assert_eq!(walz::compile(quoted).unwrap().wat, quoted);
}

#[test]
fn file_import_aliases_and_literal_name_precedence_survive_loading() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.walz"),
        r#"(module
      (@export (func $id) (func $id<i32>))
      (func $id (@type.params $T) (param $x $T) (result $T) $x)
      (func $id<i32> (result i32) (@effect pure) 42))"#,
    )
    .unwrap();
    let path = dir.path().join("main.walz");
    std::fs::write(
        &path,
        r#"(module
      (@use "./lib.walz" (func $id $identity) (func $id<i32>))
      (func $main (export "main") (result i32) (@effect pure)
        (call $identity<i32> (call $id<i32>))))"#,
    )
    .unwrap();
    let source = walz::source::load(&path).unwrap();
    walz::compile(source.text()).unwrap();
}
