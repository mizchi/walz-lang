fn source(alias: bool) -> String {
    let source = r#"(module
      (%use "wlt:core" (type $dict) (func $dict.new) (func $dict.get))
      (func $f (%effect reads writes allocates)
        (%let $d (call (%inst $dict.new i64))
          (drop (call (%inst $dict.get i32) $d (%string.lit "key"))))))"#;
    if alias {
        source
            .replace("(type $dict)", "(type $dict $Map)")
            .replace("(func $dict.new)", "(func $dict.new $make)")
            .replace("(func $dict.get)", "(func $dict.get $lookup)")
            .replace("%inst $dict.new", "%inst $make")
            .replace("%inst $dict.get", "%inst $lookup")
    } else {
        source.into()
    }
}
#[test]
fn monomorphized_errors_show_source_types_arguments_and_local_aliases() {
    for alias in [false, true] {
        let error = wlt::check(&source(alias)).unwrap_err();
        let d = error.diagnostic();
        assert_eq!(d.code, "E_TYPE_MISMATCH");
        let container = if alias { "$Map" } else { "$dict" };
        assert!(d.message.contains(&format!("{container}<i32>")), "{d:?}");
        assert!(d.message.contains(&format!("{container}<i64>")), "{d:?}");
        let function = if alias { "$lookup" } else { "$dict.get" };
        assert!(
            d.related
                .iter()
                .any(|l| l.message.contains(&format!("{function}<i32>"))),
            "{d:?}"
        );
        assert!(!error.to_string().contains(".mono"), "{d:?}");
        assert!(!error.to_string().contains("$wlt-core"), "{d:?}");
    }
}
#[test]
fn nested_generic_names_are_exact_and_user_written_mono_suffixes_are_not_erased() {
    let source = r#"(module (type $Box (%type.params $T) (struct (field $value $T)))
      (func $take (param $x (ref (%inst $Box (ref (%inst $Box i32))))))
      (func $bad (param $x (ref (%inst $Box (ref (%inst $Box i64))))) (call $take $x)))"#;
    let error = wlt::check(source).unwrap_err().to_string();
    assert!(error.contains("$Box<i32>"), "{error}");
    assert!(error.contains("$Box<i64>"), "{error}");
    assert!(!error.contains(".mono"), "{error}");
    let source = "(module (type $A.mono1 (struct)) (type $A.mono10 (struct)) (func $f (param $a (ref $A.mono1)) (result (ref $A.mono10)) $a))";
    let error = wlt::check(source).unwrap_err().to_string();
    assert!(error.contains("$A.mono1"));
    assert!(error.contains("$A.mono10"));
}
