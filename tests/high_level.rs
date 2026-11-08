use std::process::Command;

const SOURCE: &str = include_str!("component-fixtures/high-level.wlt");

#[test]
fn shared_list_patterns_field_snapshots_and_updates_execute_in_both_optimization_modes() {
    for optimize in [false, true] {
        let compiled = wlt::compile_with_options(SOURCE, wlt::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        for (entry, expected) in [
            ("main", "42"),
            ("snapshots", "1230"),
            ("throw-update", "12"),
            ("nested", "18"),
        ] {
            let out = Command::new("wasmtime")
                .args([
                    "run",
                    "-W",
                    "gc=y,function-references=y,exceptions=y",
                    "--invoke",
                    entry,
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                expected,
                "{entry}, optimize={optimize}"
            );
        }
    }
    wlt::component::compile(SOURCE).unwrap();
    wlt::component::core_js::compile(SOURCE).unwrap();
}

#[test]
fn for_each_rejects_wrong_collections_patterns_results_and_repeated_moves() {
    for (body, message) in [
        ("(%for-each $x (i32.const 1) (nop))", "list"),
        ("(%for-each $x (%list.new i32 1) $x)", "type mismatch"),
        ("(%for-each ($x $y) (%list.new i32 1) (nop))", "pair"),
        (
            "(%for-each ($x $x) (%list.new (%tuple i32 i32) (%tuple.new i32 i32 1 2)) (nop))",
            "duplicate",
        ),
        ("(%for-each $x (%list.new %unit (nop)) (nop))", "non-unit"),
        (
            "(%let $box (%box.new i32 1) (%for-each $x (%list.new i32 1 2) (%box.close (%move $box))))",
            "ownership",
        ),
    ] {
        let source = format!("(module (func $f {body}))");
        let error = wlt::check(&source).unwrap_err();
        assert!(error.to_string().contains(message), "{body}: {error}");
        assert!(error.diagnostic().primary.is_some());
    }
    let source = "(module (func $f (param $xs (%list (%box i32))) (%for-each $x $xs (%box.close (%move $x)))))";
    assert!(wlt::check(source).is_err());
}

#[test]
fn field_patterns_reject_duplicates_unknown_fields_invalid_binders_and_nullable_subjects() {
    for (pattern, subject, message) in [
        ("{:x $a :x $b}", "$s", "duplicate field"),
        ("{:x $a :y $a}", "$s", "duplicate"),
        ("{:missing $a}", "$s", "unknown field"),
        ("{:x 1}", "$s", "identifier"),
        ("{:x $a}", "(ref.null $S)", "non-null"),
        ("{:x $a}", "(i32.const 1)", "struct"),
    ] {
        let source = format!(
            "(module (type $S (struct (field $x i32) (field $y (mut i32)))) (func $f (param $s (ref $S)) (%let {pattern} {subject} (nop))))"
        );
        let error = wlt::check(&source).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert!(error.diagnostic().primary.is_some());
    }
}

#[test]
fn updates_enforce_mutability_type_effect_and_binder_scope() {
    for (update, effect, message) in [
        ("(%struct.update $S $s ($x $old 2))", "", "immutable"),
        (
            "(%struct.update $S $s ($y $old (i64.const 2)))",
            "",
            "type mismatch",
        ),
        (
            "(%struct.update $S $s ($y $old 2))",
            "(%effect pure)",
            "effect",
        ),
        (
            "(%struct.update $S $s ($y $old 2))",
            "(%effect writes)",
            "effect",
        ),
        (
            "(%struct.update $S $s ($missing $old 2))",
            "",
            "unknown field",
        ),
        (
            "(block (%struct.update $S $s ($y $old 2)) (drop $old))",
            "",
            "unbound",
        ),
        (
            "(%struct.update $S $s ($y $old 2) ($y $old 3))",
            "",
            "expected",
        ),
    ] {
        let source = format!(
            "(module (type $S (struct (field $x i32) (field $y (mut i32)))) (func $f (param $s (ref $S)) {effect} {update}))"
        );
        let error = wlt::check(&source).unwrap_err();
        assert!(error.to_string().contains(message), "{update}: {error}");
    }
}

#[test]
fn generic_type_arguments_and_field_names_have_separate_roles() {
    let source = r#"(module
      (type $Box (%type.params $T) (%where (shared $T) (nonunit $T)) (struct (field $T (mut $T))))
      (func $id (%type.params $T) (%where (shared $T) (nonunit $T)) (param $s (ref (%inst $Box $T))) (result $T)
        (%let {:T $T} $s (block (result $T) (%struct.update (%inst $Box $T) $s ($T $old $T)) $T)))
      (func $f (result i32) (call (%inst $id i32) (%struct.new (%inst $Box i32) ($T 42)))))"#;
    wlt::compile(source).unwrap();
    let source = r#"(module (func $walk (%type.params $T) (%where (shared $T) (nonunit $T)) (param $xs (%list $T))
      (%for-each $T $xs (drop $T))) (func $main (call (%inst $walk i32) (%list.new i32 1 2))))"#;
    wlt::compile(source).unwrap();
}

#[test]
fn hidden_temporaries_do_not_enter_source_scope_and_outer_exit_labels_still_work() {
    let source = r#"(module (func $f (result i32) (%effect pure)
      (%let $high.0 (type i32) 42
        (block $done (result i32)
          (%for-each $x (%list.new i32 1 2) (br $done $high.0)) 0))))"#;
    wlt::compile(source).unwrap();
    wlt::verified::compile(source).unwrap();
    let bad = "(module (func $f (%loop $outer (bind $n i32 0) (%for-each $x (%list.new i32 1) (%recur $outer 1)))))";
    assert!(wlt::check(bad).is_err());
}

#[test]
fn formatter_preserves_field_patterns_and_is_idempotent() {
    let formatted = wlt::format::source(SOURCE).unwrap();
    assert!(formatted.contains(":x $a"));
    assert_eq!(wlt::format::source(&formatted).unwrap(), formatted);
    wlt::check(&formatted).unwrap();
}

#[test]
fn new_sugar_preserves_opaque_representation_boundaries() {
    for body in [
        "(%let {:x $x} $s (nop))",
        "(%struct.update $m.S $s ($x $old 2))",
    ] {
        let source = format!(
            "(module (%legacy.namespace $m (%public (opaque $S)) (type $S (struct (field $x (mut i32))))) (func $f (param $s (ref $m.S)) {body}))"
        );
        let error = wlt::check(&source).unwrap_err();
        assert_eq!(
            error.diagnostic().code,
            "E_OPAQUE_REPRESENTATION",
            "{error}"
        );
    }
}

#[test]
fn editor_tracks_new_bindings_in_checked_and_error_recovery_snapshots() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let uri = wlt::lsp::file_uri(&dir.path().join("main.wlt")).unwrap();
    let source = r#"(module (type $S (struct (field $x (mut i32))))
      (func $f (param $s (ref $S))
        (%let {:x $amount} $s
          (%for-each ($value $weight) (%list.new (%tuple i32 i32) (%tuple.new i32 i32 1 2))
            (%struct.update $S $s ($x $old (+ $old (+ $value (+ $weight $amount)))))))))"#;
    let position = |text: &str, at: usize| {
        let before = &text[..at];
        json!({"line": before.bytes().filter(|&b| b == b'\n').count(), "character": before.rsplit('\n').next().unwrap().encode_utf16().count()})
    };
    let mut session = wlt::lsp::Session::new(Default::default());
    for (version, text) in [
        (1, source.to_string()),
        (
            2,
            source.replace("(+ $weight $amount)", "(+ $weight (i64.const 1))"),
        ),
    ] {
        session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":version,"text":text}}}));
        for name in ["$amount", "$value", "$weight", "$old"] {
            let at = text.rfind(name).unwrap();
            // In the broken revision $amount no longer occurs as a use.
            if version == 2 && name == "$amount" {
                continue;
            }
            let response = session.handle(json!({"id":1,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":position(&text, at)}}));
            let definitions = response[0]["result"].as_array().expect("definitions");
            assert_eq!(definitions.len(), 1, "{version}: {name}: {response:?}");
            assert_eq!(
                definitions[0]["range"]["start"],
                position(&text, text.find(name).unwrap()),
                "{version}: {name}: {response:?}"
            );
        }
    }
}

#[test]
fn abrupt_and_nullable_update_targets_remain_valid_wasm() {
    let source = r#"(module (type $S (struct (field $x (mut i32))))
      (func $f (result i32) (block (result i32)
        (%struct.update $S (return 42) ($x $old 1)) 0))
      (func $g (param $s (ref null $S)) (%struct.update $S $s ($x $old (+ $old 1)))))"#;
    wlt::compile(source).unwrap();
}

#[test]
fn per_iteration_owners_are_checked_by_the_existing_verified_contract() {
    let source = r#"(module (type $S (struct (field $n (mut i32))))
      (func $main (export "main") (result i32) (%effect reads writes allocates)
        (%let $s (%$S::{:n 0})
          (block (result i32)
            (%for-each $x (%list.new i32 10 32)
              (%let $owner (%box.new i32 $x)
                (%match (%box i32) (%move $owner)
                  (case own ($value) (%struct.update $S $s ($n $old (+ $old $value)))))))
            (struct.get $S $n $s)))))"#;
    wlt::verified::compile(source).unwrap();
    let source = r#"(module (import "host" "work" (func $work (%effect host)))
      (func $f (%effect pure) (%for-each $x (%list.new i32) (call $work))))"#;
    assert!(
        wlt::check(source)
            .unwrap_err()
            .to_string()
            .contains("effect")
    );
}
