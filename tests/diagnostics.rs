use gcwat::{Error, check};

fn rejects(source: &str, code: &str) -> Error {
    let error = check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, code, "{error}");
    assert!(error.diagnostic().primary.is_some(), "{error}");
    error
}

#[test]
fn call_arity_counts_only_source_arguments_and_names_the_callee() {
    let source = "(module\n  (func $add (param $a i32) (param $b i32) (result i32) (i32.add (local.get $a) (local.get $b)))\n  (func $main (result i32) (call $add (i32.const 1))))";
    let error = rejects(source, "E_CALL_ARITY");
    let text = error.to_string();
    assert!(text.contains("`$add`: expected 2, got 1"), "{text}");
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.message.contains("$add") && label.location.line == 2)
    );
    let closure = "(module (closure $C (capture $x i32) (param $y i32) (result i32) (local.get $y)) (func $f (param $c (ref $C)) (result i32) (closure.call $C (local.get $c))))";
    let error = rejects(closure, "E_CALL_ARITY");
    assert!(error.to_string().contains("closure environment"));
}

#[test]
fn unknown_local_suggests_only_visible_bindings_and_limits_capture_advice() {
    let error = rejects(
        "(module (func $f (param $value i32) (result i32) (local.get $vale)))",
        "E_UNKNOWN_LOCAL",
    );
    assert!(error.to_string().contains("did you mean `$value`"));
    assert!(!error.to_string().contains("captures must be explicit"));
    let error = rejects(
        "(module (func $hidden (param $secret i32) (nop)) (func $f (result i32) (local.get $secre)))",
        "E_UNKNOWN_LOCAL",
    );
    assert!(error.diagnostic().help.is_empty());
    let error = rejects(
        "(module (closure $C (result i32) (local.get $missing)))",
        "E_UNKNOWN_LOCAL",
    );
    assert!(error.to_string().contains("captures must be explicit"));
}

#[test]
fn type_errors_explain_numeric_operand_parameter_and_declared_result_origins() {
    let error = rejects(
        "(module (func $f (param $x i32) (result i64) (i64.add (local.get $x) (i64.const 1))))",
        "E_TYPE_MISMATCH",
    );
    assert!(error.to_string().contains("operand 1 of `i64.add`"));
    let error = rejects(
        "(module\n (func $f (param $x i64) (result i64) (local.get $x))\n (func $g (result i64) (call $f (i32.const 1))))",
        "E_TYPE_MISMATCH",
    );
    assert!(error.to_string().contains("argument 1 of `$f`"));
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 2 && label.message.contains("$x"))
    );
    let error = rejects(
        "(module\n (func $f (result i64)\n  (i32.const 1)))",
        "E_TYPE_MISMATCH",
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 2 && label.message.contains("result"))
    );
    assert_eq!(
        error.diagnostic().primary.as_ref().unwrap().location.line,
        3
    );
}

#[test]
fn moved_value_diagnostic_identifies_binding_and_first_move() {
    let source = "(module (func $f\n (let $owner (own.new i32 (i32.const 1))\n  (block\n   (own.close (move $owner))\n   (own.close (move $owner))))))";
    let error = rejects(source, "E_OWNERSHIP_MOVED");
    assert!(error.to_string().contains("`$owner`"));
    assert_eq!(
        error.diagnostic().primary.as_ref().unwrap().location.line,
        5
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 4 && label.message.contains("moved"))
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 2 && label.message.contains("bound"))
    );
}

#[test]
fn effect_diagnostic_explains_declared_bound_and_transitive_cause() {
    let source = "(module\n (type $Cell (struct (field $value (mut i32))))\n (func $read (param $cell (ref $Cell)) (result i32)\n  (struct.get $Cell $value (local.get $cell)))\n (func $f (param $cell (ref $Cell)) (result i32) (effects pure)\n  (call $read (local.get $cell))))";
    let error = rejects(source, "E_EFFECT_CONTRACT");
    assert!(error.to_string().contains("declared: pure"));
    assert!(error.to_string().contains("required: reads"));
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 6 && label.message.contains("$read"))
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 4 && label.message.contains("reads"))
    );
}

#[test]
fn opaque_diagnostic_identifies_type_owner_and_declaration() {
    let source = "(module\n (namespace $lib\n  (public (opaque $State))\n  (type $State (struct (field $n i32))))\n (func $read (param $s (ref $lib.State)) (result i32)\n  (struct.get $lib.State $n (local.get $s))))";
    let error = rejects(source, "E_OPAQUE_REPRESENTATION");
    assert!(error.to_string().contains("`$lib.State`"));
    assert!(error.to_string().contains("namespace `$lib`"));
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 4)
    );
}

#[test]
fn included_primary_and_related_spans_map_independently_with_unicode_columns() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("lib.gwat"),
        "(module\n (func $f (param $value i64) (result i64) (local.get $value)))",
    )
    .unwrap();
    let program = directory.path().join("main.gwat");
    std::fs::write(&program, "(module\n (include \"lib.gwat\")\n (func $main (result i64) (; 日本語 ;) (call $f (i32.const 1))))").unwrap();
    let source = gcwat::source::load(&program).unwrap();
    let error = source.diagnostic(check(source.text()).unwrap_err());
    let diagnostic = error.diagnostic();
    let primary = diagnostic.primary.as_ref().unwrap();
    assert_eq!(
        primary.location.path.as_ref().unwrap().file_name().unwrap(),
        "main.gwat"
    );
    assert_eq!(primary.location.line, 3);
    assert_eq!(primary.location.column, 46);
    let declaration = diagnostic
        .related
        .iter()
        .find(|label| label.message.contains("$value"))
        .unwrap();
    assert_eq!(
        declaration
            .location
            .path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap(),
        "lib.gwat"
    );
    assert_eq!(declaration.location.line, 2);
    let json = serde_json::to_value(diagnostic).unwrap();
    assert_eq!(json["code"], "E_TYPE_MISMATCH");
    assert!(error.to_string().contains("lib.gwat:2:"));
}

#[test]
fn branch_move_provenance_retains_each_reachable_origin() {
    let source = "(module (func $f (param $flag i32)\n (let $owner (own.new i32 (i32.const 1))\n  (block\n   (if (local.get $flag)\n    (then (own.close (move $owner)))\n    (else (own.close (move $owner))))\n   (own.close (move $owner))))))";
    let error = rejects(source, "E_OWNERSHIP_MOVED");
    let origins = error
        .diagnostic()
        .related
        .iter()
        .filter(|label| label.message.contains("moved"))
        .collect::<Vec<_>>();
    assert_eq!(
        origins
            .iter()
            .map(|label| label.location.line)
            .collect::<Vec<_>>(),
        [5, 6]
    );
    assert!(origins.iter().all(|label| label.message.contains("branch")));
    // Removing only the final second use makes the branch join valid.
    check(&source.replace("\n   (own.close (move $owner))", "\n   (nop)")).unwrap();
}

#[test]
fn inner_type_errors_do_not_blame_an_outer_call_or_result_bound() {
    let source = "(module\n (func $id (param $value i32) (result i32) (local.get $value))\n (func $f (result i32)\n  (call $id (i32.add (i64.const 1) (i32.const 2)))))";
    let error = rejects(source, "E_TYPE_MISMATCH");
    assert!(error.to_string().contains("operand 1 of `i32.add`"));
    assert!(!error.to_string().contains("argument 1 of `$id`"));
    assert!(!error.to_string().contains("result of `$f`"));
}

#[test]
fn effect_provenance_ignores_caught_throws_and_finds_handler_rethrows() {
    let source = "(module (tag $E (param i32))\n (func $f (result i32) (effects pure)\n  (try (result i32)\n   (do (throw $E (i32.const 1)))\n   (catch $E ($value) (throw $E (local.get $value))))))";
    let error = rejects(source, "E_EFFECT_CONTRACT");
    assert_eq!(error.diagnostic().related.len(), 1);
    assert_eq!(error.diagnostic().related[0].location.line, 5);
    assert!(error.diagnostic().related[0].message.contains("throws($E)"));
}

#[test]
fn effect_provenance_handles_recursive_calls_and_unknown_imports() {
    let source = "(module\n (type $Cell (struct (field (mut i32))))\n (func $a (param $c (ref $Cell)) (effects pure) (call $b (local.get $c)))\n (func $b (param $c (ref $Cell))\n  (block (call $a (local.get $c))\n   (struct.set $Cell 0 (local.get $c) (i32.const 1)))))";
    let error = rejects(source, "E_EFFECT_CONTRACT");
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 6 && label.message.contains("struct.set"))
    );
    let error = rejects(
        "(module (import \"host\" \"f\" (func $host)) (func $f (effects pure) (call $host)))",
        "E_EFFECT_CONTRACT",
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.message.contains("call to `$host`"))
    );
}

#[test]
fn included_cli_errors_keep_related_locations_and_do_not_panic_at_eof() {
    let directory = tempfile::tempdir().unwrap();
    let library = directory.path().join("lib.gwat");
    let program = directory.path().join("main.gwat");
    std::fs::write(
        &library,
        "(module\n (func $f (param $value i64) (result i64) (local.get $value)))",
    )
    .unwrap();
    std::fs::write(
        &program,
        "(module\n (include \"lib.gwat\")\n (func $main (result i64) (call $f (i32.const 1))))",
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gcwat"))
        .arg("check")
        .arg(&program)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("main.gwat:3:"), "{stderr}");
    assert!(stderr.contains("lib.gwat:2:"), "{stderr}");
    std::fs::write(&library, "(module (;").unwrap();
    let error = match gcwat::source::load(&program) {
        Ok(_) => panic!("accepted unclosed comment"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("lib.gwat:1:11:"), "{error}");
    assert!(
        error
            .diagnostic()
            .primary
            .as_ref()
            .unwrap()
            .location
            .path
            .is_some()
    );
}

#[test]
fn numeric_repair_hint_uses_the_surrounding_type_without_implying_a_cast() {
    let error = rejects(
        "(module (func $inc (param $value i32) (result i32) (i64.add (local.get $value) (i64.const 1))))",
        "E_TYPE_MISMATCH",
    );
    let text = error.to_string();
    assert!(text.contains("surrounding context expects i32"), "{text}");
    assert!(
        text.contains("i32.add") && text.contains("including numeric constants"),
        "{text}"
    );
    // An i64 result is intentional here; do not recommend an i32 operator.
    let error = rejects(
        "(module (func $f (param $x i32) (result i64) (i64.add (local.get $x) (i64.const 1))))",
        "E_TYPE_MISMATCH",
    );
    assert!(error.diagnostic().help.is_empty(), "{error}");
}

#[test]
fn alias_cycle_reports_the_ordered_edges_and_a_non_prescriptive_repair() {
    let source = "(module\n (type.alias $Entry $Input)\n (type.alias $Input (option $Output))\n (type.alias $Output $Input))";
    let error = rejects(source, "E_ALIAS_CYCLE");
    assert!(
        error.to_string().contains("$Input -> $Output -> $Input"),
        "{error}"
    );
    assert_eq!(
        error.diagnostic().primary.as_ref().unwrap().location.line,
        4
    );
    let related = &error.diagnostic().related;
    assert!(
        related
            .iter()
            .any(|label| label.location.line == 3 && label.message.contains("$Output"))
    );
    assert!(
        related
            .iter()
            .any(|label| label.location.line == 4 && label.message.contains("$Input"))
    );
    assert!(
        error
            .to_string()
            .contains("concrete type or a type outside this cycle")
    );
    assert!(!error.to_string().contains("i32"));
}

#[test]
fn distant_unknown_names_list_visible_bindings_without_guessing_a_replacement() {
    let source = "(module\n (func $other (param $secret i32) (nop))\n (func $inc (param $value i32) (result i32)\n  (i32.add (local.get $missing) (i32.const 1))))";
    let error = rejects(source, "E_UNKNOWN_LOCAL");
    let text = error.to_string();
    assert!(text.contains("available local `$value`: i32"), "{text}");
    assert!(
        !text.contains("$secret") && !text.contains("did you mean"),
        "{text}"
    );
    assert!(
        error
            .diagnostic()
            .related
            .iter()
            .any(|label| label.location.line == 3 && label.message.contains("$value"))
    );
    assert!(
        error
            .diagnostic()
            .help
            .iter()
            .any(|help| help.contains("existing binding"))
    );
}

#[test]
fn alias_cycle_edges_keep_their_individual_include_locations() {
    let directory = tempfile::tempdir().unwrap();
    let library = directory.path().join("types.gwat");
    let program = directory.path().join("main.gwat");
    std::fs::write(&library, "(module\n (type.alias $Input $Output))").unwrap();
    std::fs::write(
        &program,
        "(module\n (include \"types.gwat\")\n (type.alias $Output $Input))",
    )
    .unwrap();
    let loaded = gcwat::source::load(&program).unwrap();
    let error = loaded.diagnostic(check(loaded.text()).unwrap_err());
    assert_eq!(error.diagnostic().code, "E_ALIAS_CYCLE");
    let primary = error.diagnostic().primary.as_ref().unwrap();
    assert_eq!(
        primary.location.path.as_ref().unwrap().file_name().unwrap(),
        "main.gwat"
    );
    assert_eq!(primary.location.line, 3);
    assert!(error.diagnostic().related.iter().any(|label| {
        label.location.path.as_ref().unwrap().file_name().unwrap() == "types.gwat"
            && label.location.line == 2
    }));
}
