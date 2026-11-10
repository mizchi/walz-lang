// Compiler scope fixtures use private expanded forms; public files use @use paths.
use std::process::Command;

fn run(source: &str, expected: &str) {
    for optimize in [false, true] {
        let compiled =
            walz::compile_with_options(source, walz::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let output = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                "main",
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn named_fields_braces_and_literal_elision_are_independent() {
    for construction in [
        "(@struct.new $Pair ($right (i32.const 2)) ($left (i32.const 40)))",
        "(@struct.new $Pair ($right 2) ($left 40))",
        "(@struct.new $Pair {:right (i32.const 2) :left (i32.const 40)})",
        "(@struct.new $Pair {:right 2 :left 40})",
        "(@$Pair::{:right (i32.const 2) :left (i32.const 40)})",
        "(@$Pair::{:right 2 :left 40})",
    ] {
        run(
            &format!(
                r#"(module
          (type $Pair (struct (field $left i32) (field $right (mut i32))))
          (func $main (export "main") (result i32)
            (@let $p {construction}
              (i32.add (struct.get $Pair $left (local.get $p))
                (struct.get $Pair $right (local.get $p))))))"#
            ),
            "42",
        );
    }
}

#[test]
fn initializers_keep_source_order_and_evaluate_once_without_capturing_locals() {
    run(
        r#"(module
      (type $Trace (struct (field $log (mut i32))))
      (type $Pair (struct (field $left i32) (field $right i32)))
      (func $mark (param $t (ref $Trace)) (param $n i32) (result i32)
        (block (result i32)
          (struct.set $Trace $log (local.get $t)
            (i32.add (i32.mul (struct.get $Trace $log (local.get $t)) (i32.const 10)) (local.get $n)))
          (local.get $n)))
      (func $main (export "main") (result i32)
        (@let (($left (i32.const 1)) ($t (struct.new $Trace (i32.const 0)))
               ($p (@$Pair::{:right (call $mark (local.get $t) (i32.const 2))
                           :left (call $mark (local.get $t) (local.get $left))})))
          (i32.add (i32.mul (struct.get $Trace $log (local.get $t)) (i32.const 100))
            (i32.add (i32.mul (struct.get $Pair $left (local.get $p)) (i32.const 10))
              (struct.get $Pair $right (local.get $p)))))))"#,
        "2112",
    );
}

#[test]
fn generic_types_namespaces_and_nested_text_values_use_existing_resolution() {
    run(
        r#"(module
      (@use "walz:core" (type $string))
      (@legacy.namespace $model
        (@use "walz:core" (type $string))
        (@public (type $Box) (type $User))
        (type $Box (@type.params $T) (@where (shared $T) (nonunit $T))
          (struct (field $value $T)))
        (type $User (struct (field $name $string) (field $age i32) (field $email $string))))
      (func $main (export "main") (result i32)
        (@let $boxed (@struct.new (@inst $model.Box (ref $model.User))
          {:value (@$model.User::{:email "alice@example.com" :age 30 :name "Alice"})})
          (@let $u (struct.get (@inst $model.Box (ref $model.User)) $value (local.get $boxed))
            (i32.add (struct.get $model.User $age (local.get $u))
              (i32.add (@string.byte-len (struct.get $model.User $name (local.get $u)))
                (i32.const 7)))))))"#,
        "42",
    );
}

#[test]
fn named_field_errors_are_static_and_point_to_the_source() {
    for (body, message) in [
        ("(@struct.new $Pair ($left 1))", "missing"),
        ("(@$Pair::{:left 1 :left 2 :right 3})", "duplicate"),
        (
            "(@struct.new $Pair ($left 1) ($right 2) ($other 3))",
            "unknown",
        ),
        ("(@$Pair::{:left 4294967296 :right 1})", "numeric literal"),
        ("(@$Pair::{:left \"text\" :right 1})", "literal"),
    ] {
        let source = format!(
            "(module (type $Pair (struct (field $left i32) (field $right i32))) (func $f (result (ref $Pair)) {body}))"
        );
        let error = walz::compile(&source)
            .err()
            .expect("invalid constructor accepted");
        assert!(error.to_string().contains(message), "{body}: {error}");
        assert!(error.diagnostic().primary.is_some(), "{error}");
    }
}

#[test]
fn abrupt_initializers_stop_before_later_values() {
    run(
        r#"(module
      (type $Pair (struct (field $left i32) (field $right i32)))
      (func $main (export "main") (result i32)
        (struct.get $Pair $left
          (@$Pair::{:right (return (i32.const 42)) :left (unreachable)}))))"#,
        "42",
    );
}

#[test]
fn opaque_types_effects_and_anonymous_fields_keep_their_contracts() {
    for (source, message) in [
        (
            r#"(module (@legacy.namespace $n (@public (opaque $S)) (type $S (struct (field $x i32))))
          (func $f (result (ref $n.S)) (@$n.S::{:x 1})))"#,
            "opaque",
        ),
        (
            r#"(module (type $S (struct (field $x i32)))
          (func $f (result (ref $S)) (@effect pure) (@$S::{:x 1})))"#,
            "effect contract",
        ),
        (
            r#"(module (type $S (struct (field i32)))
          (func $f (result (ref $S)) (@struct.new $S ($x 1))))"#,
            "every field",
        ),
    ] {
        let error = walz::check(source).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
    let native = r#"(module (func $"{}::" (export "x") (result i32) (i32.const 42))
      (memory 1) (data (i32.const 0) "(@User::{:age 30})"))"#;
    assert_eq!(walz::compile(native).unwrap().wat, native);
}

#[test]
fn constructor_markers_inside_native_annotations_are_not_reinterpreted() {
    let source = r#"(module (@meta (@User::))
      (func (export "main") (result i32) (i32.const 42)))"#;
    wat::parse_str(source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("annotation.wat");
    std::fs::write(&path, source).unwrap();
    let loaded = walz::source::load(&path).unwrap();
    assert_eq!(walz::compile(loaded.text()).unwrap().wat, source);
}

#[test]
fn named_construction_preserves_gc_identity_and_literal_string_contents() {
    run(
        r#"(module (type $Empty (struct))
      (func $main (export "main") (result i32) (ref.eq (@$Empty::{}) (@$Empty::{}))))"#,
        "0",
    );
    run(
        r#"(module (@use "walz:core" (type $string)) (type $S (struct (field $x $string)))
      (func $main (export "main") (result i32)
        (i32.add (i32.const 38) (@string.byte-len (struct.get $S $x (@$S::{:x "{::}"}))))))"#,
        "42",
    );
}

#[test]
fn formatter_keeps_field_groups_comments_and_quoted_identifiers() {
    let source = r#"(module
      (type $Pair (struct (field $left i32) (field $right i32)))
      (func $main (export "main") (result i32)
        (@let $p (@$Pair::{:right 2 ;; keep the initializer with its field
                         :left (; {} :: nested (; comment ;) ;) 40})
          (i32.add (struct.get $Pair $left (local.get $p))
            (struct.get $Pair $right (local.get $p))))))"#;
    let formatted = walz::format::source(source).unwrap();
    assert!(formatted.contains(":left"));
    assert!(formatted.contains(":right 2"), "{formatted}");
    assert!(formatted.contains("(; {} :: nested (; comment ;) ;)"));
    assert_eq!(walz::format::source(&formatted).unwrap(), formatted);
    run(&formatted, "42");
    let native = r#"(module (func $"{}::" (export "main") (result i32) (i32.const 42)))"#;
    let formatted = walz::format::source(native).unwrap();
    assert!(formatted.contains("$\"{}::\""), "{formatted}");
    assert_eq!(
        walz::compile(native).unwrap().wasm,
        walz::compile(&formatted).unwrap().wasm
    );
}

#[test]
fn numeric_literals_follow_the_declared_wasm_type_without_conversion() {
    run(
        r#"(module
      (type $N (struct (field $a i32) (field $b i64) (field $c f32) (field $d f64)))
      (func $main (export "main") (result i32)
        (@let $n (@$N::{:d 2e0 :c 0x1.5p+5 :b 18446744073709551615 :a 4294967295})
          (i32.add (struct.get $N $a (local.get $n))
            (i32.add (i32.wrap_i64 (struct.get $N $b (local.get $n)))
              (i32.add (i32.trunc_f32_s (struct.get $N $c (local.get $n)))
                (i32.trunc_f64_s (struct.get $N $d (local.get $n)))))))))"#,
        "42",
    );
    run(
        r#"(module (type $N (struct (field $a f32) (field $b f64)))
      (func $main (export "main") (result i32)
        (@let $n (@struct.new $N ($a nan:0x42) ($b -inf))
          (i32.and (f32.ne (struct.get $N $a (local.get $n)) (struct.get $N $a (local.get $n)))
            (f64.eq (struct.get $N $b (local.get $n)) (f64.const -inf))))))"#,
        "1",
    );
}

#[test]
fn short_literals_do_not_add_defaults_or_implicit_wraps_and_coercions() {
    for source in [
        r#"(module (type $Id (@newtype i32)) (type $S (struct (field $x $Id))) (func $f (result (ref $S)) (@$S::{:x 1})))"#,
        r#"(module (@use "walz:core" (type $bytes)) (type $S (struct (field $x $bytes))) (func $f (result (ref $S)) (@$S::{:x "a"})))"#,
        r#"(module (@use "walz:core" (type $string)) (type $S (struct (field $x $string))) (func $f (result (ref $S)) (@$S::{:x "\ff"})))"#,
        r#"(module (type $S (struct (field $x v128))) (func $f (result (ref $S)) (@$S::{:x 1})))"#,
        r#"(module (type $S (struct (field $x (ref null $S)))) (func $f (result (ref $S)) (@$S::{})))"#,
        r#"(module (type $S (struct (field $x i32))) (func $f (result (ref $S)) (@$S::{:x null})))"#,
    ] {
        assert!(walz::compile(source).is_err(), "{source}");
    }
    run(
        r#"(module (type $Id (@newtype i32)) (type $S (struct (field $x $Id)))
      (func $main (export "main") (result i32)
        (@newtype.unwrap $Id (struct.get $S $x
          (@$S::{:x (@newtype.wrap $Id (i32.const 42))})))))"#,
        "42",
    );
}

#[test]
fn empty_structs_malformed_maps_and_field_names_have_separate_roles() {
    run(
        r#"(module (type $Empty (struct))
      (type $Box (@type.params $T) (@where (shared $T) (nonunit $T)) (struct (field $T $T)))
      (func $main (export "main") (result i32)
        (block (result i32) (drop (@$Empty::{}))
          (struct.get (@inst $Box i32) $T (@struct.new (@inst $Box i32) ($T 42))))))"#,
        "42",
    );
    for body in [
        "(@$S::{:x})",
        "(@$S::{x 1})",
        "(@$S::{:x 1)",
        "(@$S::{:x 1} ($x 2))",
        "(@struct.new $S ($x 1) {})",
        "(@$S::{:x \"a\":y 1})",
    ] {
        let source = format!(
            "(module (type $S (struct (field $x i32))) (func $f (result (ref $S)) {body}))"
        );
        assert!(walz::compile(&source).is_err(), "{source}");
    }
}

#[test]
fn initializer_exceptions_keep_the_existing_ownership_proof_contract() {
    let source = r#"(module (tag $E)
      (type $Pair (struct (field $left i32) (field $right i32)))
      (func $fail (param $flag i32) (result i32) (@effect (throws $E))
        (if (result i32) (local.get $flag) (then (throw $E)) (else (i32.const 0))))
      (func $run (param $flag i32) (result i32) (@effect allocates)
        (@let $owner (@box.new i32 (i32.const 42))
          (@try (result i32)
            (do (@let $p (@$Pair::{:right (call $fail (local.get $flag))
              :left (@match (@box i32) (result i32) (@move $owner) (case own ($x) (local.get $x)))})
              (i32.add (struct.get $Pair $left (local.get $p)) (struct.get $Pair $right (local.get $p)))))
            (catch $E () (@match (@box i32) (result i32) (@move $owner) (case own ($x) (local.get $x)))))))
      (func $main (export "main") (result i32) (@effect allocates)
        (i32.add (call $run (i32.const 0)) (call $run (i32.const 1)))))"#;
    run(source, "84");
    walz::verified::compile(source).unwrap();
    let invalid = source.replace(
        ":right (call $fail (local.get $flag))\n              :left (@match (@box i32) (result i32) (@move $owner) (case own ($x) (local.get $x)))",
        ":left (@match (@box i32) (result i32) (@move $owner) (case own ($x) (local.get $x))) :right (call $fail (local.get $flag))",
    );
    assert_ne!(invalid, source);
    assert!(walz::check(&invalid).is_err());
}

#[test]
fn unreachable_initializers_still_undergo_effect_validation() {
    let source = r#"(module (type $S (struct (field $x i32) (field $y i32)))
      (import "host" "work" (func $work (result i32) (@effect host)))
      (func $main (result i32) (@effect pure)
        (struct.get $S $x (@$S::{:x (return (i32.const 42)) :y (call $work)}))))"#;
    let error = walz::check(source).unwrap_err().to_string();
    assert!(error.contains("effect"), "{error}");
}

#[test]
fn adjacent_reserved_string_tokens_are_not_silently_coerced_or_split_by_formatting() {
    for value in [r#""a""b""#, r#""a"suffix"#, r#""a":next"#] {
        let source = format!(
            r#"(module (@use "walz:core" (type $string))
          (type $S (struct (field $x $string)))
          (func $f (result (ref $S)) (@$S::{{:x {value}}})))"#
        );
        for input in [source.clone(), walz::format::source(&source).unwrap()] {
            assert_eq!(
                walz::check(&input).unwrap_err().diagnostic().code,
                "E_LEX_TOKEN",
                "{input}"
            );
        }
    }
}

#[test]
fn generic_functions_and_import_aliases_do_not_rewrite_field_labels() {
    run(
        r#"(module
      (@legacy.namespace $m
        (@public (type $Box))
        (type $Box (@type.params $T) (@where (shared $T) (nonunit $T)) (struct (field $T $T))))
      (@file.use $m (type $Box $Alias))
      (func $box (@type.params $T) (@where (shared $T) (nonunit $T))
        (param $value $T) (result (ref (@inst $Alias $T))) (@effect allocates)
        (@struct.new (@inst $Alias $T) ($T (local.get $value))))
      (func $main (export "main") (result i32) (@effect allocates)
        (struct.get (@inst $m.Box i32) $T
          (call (@inst $box i32) (i32.const 42)))))"#,
        "42",
    );
}

#[test]
fn compact_construction_keeps_the_dollar_type_identifier() {
    let source = r#"(module (type $User (struct (field $age i32)))
      (func $main (export "main") (result i32)
        (struct.get $User $age (@$User::{:age 30}))))"#;
    run(source, "30");
    let old = source.replace("@$User::", "@User::");
    let error = walz::compile(&old).err().expect("missing $ was accepted");
    assert_eq!(error.diagnostic().code, "E_STRUCT_LITERAL");
    assert!(error.to_string().contains("$type"), "{error}");
}

#[test]
fn bare_constants_elaborate_from_the_field_type_to_the_same_wasm() {
    let source = r#"(module (type $N (struct (field $small i32) (field $large i64)))
      (func $f (export "f") (result (ref $N))
        (@$N::{:small 30 :large 30})))"#;
    let explicit = source
        .replace(":small 30", ":small (i32.const 30)")
        .replace(":large 30", ":large (i64.const 30)");
    for optimize in [false, true] {
        let options = walz::CompileOptions { optimize };
        let bare = walz::compile_with_options(source, options).unwrap();
        let spelled = walz::compile_with_options(&explicit, options).unwrap();
        assert_eq!(bare.wasm, spelled.wasm);
        assert!(bare.wat.contains("(i32.const 30)"));
        assert!(bare.wat.contains("(i64.const 30)"));
    }
    let wrong = explicit.replace(":large (i64.const 30)", ":large (i32.const 30)");
    let error = walz::check(&wrong).unwrap_err().to_string();
    assert!(error.contains("type mismatch"), "{error}");
}
