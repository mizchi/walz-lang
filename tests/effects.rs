use walz::check;

#[test]
fn contracts_check_transitive_mutation_and_do_not_trust_import_annotations() {
    for (source, expected) in [
        (
            "(module (type $C (struct)) (func $f (result (@option (ref $C))) (@effect pure) (@option.some (ref $C) (struct.new $C))))",
            "allocates",
        ),
        (
            "(module (type $C (struct (field (mut i64)))) (func $f (param $c (ref $C)) (result (@option i64)) (@effect pure) (@option.some i64 (struct.get $C 0 (local.get $c)))))",
            "reads",
        ),
        (
            "(module (type $C (struct (field (mut i64)))) (func $write (param $c (ref $C)) (struct.set $C 0 (local.get $c) (i64.const 1))) (func $f (param $c (ref $C)) (@effect pure) (call $write (local.get $c))))",
            "writes",
        ),
        (
            "(module (type $C (struct)) (func $f (result (ref $C)) (@effect pure) (struct.new $C)))",
            "allocates",
        ),
        (
            "(module (import \"host\" \"f\" (func $f (@effect pure))))",
            "imports cannot declare effect contracts",
        ),
        (
            "(module (import \"host\" \"f\" (func $host)) (func $f (@effect pure) (call $host)))",
            "suspends",
        ),
    ] {
        let error = check(source).unwrap_err().to_string();
        assert!(error.contains(expected), "{source}\n{error}");
        assert!(error.starts_with("1:"), "{error}");
    }
    check("(module (type $C (struct (field (mut i64)))) (func $f (param $c (ref $C)) (@effect writes) (struct.set $C 0 (local.get $c) (i64.const 1))))").unwrap();
}

#[test]
fn throws_contracts_track_escaping_tags_and_handler_rethrows() {
    check(
        r#"(module (tag $E (param i64))
      (func $raise (result i64) (@effect (throws $E)) (throw $E (i64.const 42)))
      (func $caught (result i64) (@effect pure)
        (@try (result i64) (do (call $raise)) (catch $E ($n) (local.get $n)))))"#,
    )
    .unwrap();
    for body in [
        "(throw $E (i64.const 42))",
        "(@try (result i64) (do (throw $E (i64.const 1))) (catch $E ($n) (throw $E (local.get $n))))",
    ] {
        let source =
            format!("(module (tag $E (param i64)) (func $f (result i64) (@effect pure) {body}))");
        let error = check(&source).unwrap_err().to_string();
        assert!(
            error.contains("effect contract") && error.contains("$E"),
            "{error}"
        );
    }
}

#[test]
fn recursive_call_graph_cannot_hide_an_effect() {
    let error = check(
        r#"(module (type $C (struct (field (mut i64))))
      (func $a (param $c (ref $C)) (@effect pure) (call $b (local.get $c)))
      (func $b (param $c (ref $C))
        (block (struct.set $C 0 (local.get $c) (i64.const 1)) (call $a (local.get $c)))))"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("effect contract") && error.contains("writes"),
        "{error}"
    );
}

#[test]
fn malformed_contracts_are_rejected() {
    for (clause, expected) in [
        ("(@effect reads reads)", "duplicate effect"),
        ("(@effect pure writes)", "pure must be used alone"),
        ("(@effect magic)", "unknown effect"),
        ("(@effect (reads extra))", "expected effect name"),
        ("(@effect) (@effect)", "duplicate effect contract"),
        ("(@effect (throws $Missing))", "unknown tag"),
    ] {
        let source = format!("(module (func $f {clause} (nop)))");
        let error = check(&source).unwrap_err().to_string();
        assert!(error.contains(expected), "{source}\n{error}");
    }
}
