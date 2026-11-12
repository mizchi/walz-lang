use super::*;
fn same(left: &Sexp, right: &Sexp) -> bool {
    match (&left.kind, &right.kind) {
        (Kind::Atom(a), Kind::Atom(b)) | (Kind::Text(a), Kind::Text(b)) => a == b,
        (Kind::Bytes(a), Kind::Bytes(b)) => a == b,
        (Kind::List(a), Kind::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        _ => false,
    }
}
fn files(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, out);
        } else if path.extension().is_some_and(|e| e == "walz") {
            out.push(path);
        }
    }
}
#[test]
fn active_programs_roundtrip_their_exact_form_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    for path in [
        "examples",
        "walz/core",
        "templates/app",
        "tests/component-fixtures",
        "tests/programs",
    ] {
        files(&root.join(path), &mut paths);
    }
    assert!(paths.len() >= 150);
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let original = crate::sexpr::parse(&source).unwrap();
        let view = to_watz(&source).unwrap();
        let translated =
            from_watz(&view).unwrap_or_else(|e| panic!("{}: {e}\n{view}", path.display()));
        let restored = crate::sexpr::parse(&translated.text).unwrap();
        assert!(
            same(&original, &restored),
            "{}\n{view}\n{}",
            path.display(),
            translated.text
        );
        assert_eq!(
            to_watz(&translated.text).unwrap(),
            view,
            "format stability: {}",
            path.display()
        );
    }
}
#[test]
fn encoded_atoms_do_not_inject_forms_and_excessive_nesting_is_rejected() {
    for source in [
        "module { sexpr.atom(\"x) (func $hidden) (x\"); }",
        "module { sexpr.form(\"x) (func $hidden) (x\"); }",
        "module { const $x = 1; }",
        "module { func $f(): i32 { const $x = 1; return $x; } }",
    ] {
        assert!(from_watz(source).is_err(), "{source}");
    }
    let source = format!(
        "module {{ func $f(): i32 {{ {}return 1;{} }} }}",
        "{".repeat(1024),
        "}".repeat(1024)
    );
    assert!(from_watz(&source).is_err());
}
#[test]
fn reserved_spelling_and_nested_generic_types_roundtrip() {
    let source = "(module (@meta wat.foo walz.foo wat. walz.) (func $f (param $x (@inst $Box (@inst $Box i32))) (result (ref null $T)) (ref.null $T)))";
    let view = to_watz(source).unwrap();
    let restored = from_watz(&view).unwrap();
    assert!(
        same(
            &crate::sexpr::parse(source).unwrap(),
            &crate::sexpr::parse(&restored.text).unwrap()
        ),
        "{view}"
    );
}

#[test]
fn shadowing_keeps_explicit_let_and_comment_lines_never_become_forms() {
    let source = "(module (func $f (param $x i32) (result i32) (@let $x (type i32) (+ $x 1) $x)))";
    let view = to_watz(source).unwrap();
    assert!(view.contains("walz.let($x"), "{view}");
    let roundtrip = from_watz(&view).unwrap();
    assert!(same(
        &crate::sexpr::parse(source).unwrap(),
        &crate::sexpr::parse(&roundtrip.text).unwrap()
    ));
    let comments = from_watz("// note\rwat.func($hidden);\nmodule { wat.memory(1); }").unwrap();
    assert_eq!(
        crate::sexpr::parse(&comments.text)
            .unwrap()
            .list()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn long_structural_calls_are_indented_for_review() {
    let arms = (0..8)
        .map(|i| format!("(@case v{i} ($value) (+ $value {i}))"))
        .collect::<String>();
    let source = format!("(module (func $read (param $x i32) (result i32) (@match $x {arms})))");
    let view = to_watz(&source).unwrap();
    assert!(
        view.contains("return match ($x) {\n    case v0($value) =>"),
        "{view}"
    );
    let restored = from_watz(&view).unwrap();
    assert!(same(
        &crate::sexpr::parse(&source).unwrap(),
        &crate::sexpr::parse(&restored.text).unwrap()
    ));
}

#[test]
fn control_expressions_preserve_result_headers_and_case_patterns() {
    for (body, spelling) in [
        (
            "(if (result i32) (> $n 0) (then $n) (else 0))",
            "if<i32> ($n > 0)",
        ),
        (
            "(if (local.get $n) (then (nop)) (else (nop)))",
            "if (wat.local.get($n))",
        ),
        ("(if (result) $n (then) (else))", "if<> ($n)"),
        ("(if (local.get $n) (then (nop)))", "if (wat.local.get($n))"),
        (
            "(if (result i32) (> $n 0) (then 42) (else (if (result i32) (= $n 0) (then 1) (else 0))))",
            "else if<i32> ($n === 0)",
        ),
        (
            "(if (result i32 i64) $n (then (call $two)) (else (call $two)))",
            "if<i32, i64> ($n)",
        ),
        (
            "(@match $answer (@case ok ($value) $value) (@default 0))",
            "match ($answer)",
        ),
        (
            "(@match (@inst $result i32 i32) (result i32) $answer (@case ok ($value) $value) (@case err ($error) 0))",
            "match<$result<i32, i32>, i32> ($answer)",
        ),
        (
            "(@match $T $answer (@case ready () (nop)) (@default (nop)))",
            "match<$T> ($answer)",
        ),
        (
            "(@match $answer (@case not-ready ($error-code) $error-code) (@case if () 0))",
            "case not-ready(sexpr.atom(\"$error-code\")) =>",
        ),
    ] {
        let source = format!("(module (func $read (param $n i32) {body}))");
        let view = to_watz(&source).unwrap();
        assert!(view.contains(spelling), "{view}");
        let restored = from_watz(&view).unwrap();
        assert!(
            same(
                &crate::sexpr::parse(&source).unwrap(),
                &crate::sexpr::parse(&restored.text).unwrap()
            ),
            "{view}"
        );
        assert_eq!(to_watz(&restored.text).unwrap(), view);
    }
    let view = "func $read($n: i32): i32 { return if<i32> ($n > 0) { $n } else if<i32> ($n === 0) { 42 } else { 0 }; }";
    let canonical = "(module (func $read (param $n i32) (result i32) (if (result i32) (> $n 0) (then $n) (else (if (result i32) (= $n 0) (then 42) (else 0))))))";
    let restored = from_watz(view).unwrap();
    assert!(same(
        &crate::sexpr::parse(canonical).unwrap(),
        &crate::sexpr::parse(&restored.text).unwrap()
    ));
    assert_eq!(
        crate::compile(canonical).unwrap().wasm,
        crate::compile(&restored.text).unwrap().wasm
    );
}

#[test]
fn controls_keep_opaque_data_and_unsupported_wat_shapes_explicit() {
    for source in [
        "(module (@custom (< if match) if match))",
        "(module (func $f (if $label (then (nop)))))",
        "(module (func $f (if $label (result i32) (local.get 0) (then 1) (else 0))))",
        "(module (func $f (if (result i32) 1 (then (nop) 42) (else 0))))",
        "(module (func $f (@match $T (result) $value (@case ready () (nop)))))",
    ] {
        let view = to_watz(source).unwrap();
        let restored = from_watz(&view).unwrap();
        assert!(
            same(
                &crate::sexpr::parse(source).unwrap(),
                &crate::sexpr::parse(&restored.text).unwrap()
            ),
            "{view}"
        );
        if source.contains("(if ") {
            assert!(view.contains("wat.if("), "{view}");
        }
    }
    for invalid in [
        "func $f(): i32 { return if<i32> (1) { 42; } else { 0 }; }",
        "func $f(): i32 { return if<i32> (1) { 42 } else ; }",
        "func $f(): i32 { return match ($v) { case ready => 1 }; }",
        "func $f(): i32 { return match ($v) { default => 1 case ready() => 0 }; }",
        "func $f(): i32 { return match<$T, i32, i64> ($v) { default => 1 }; }",
        "func $f(): i32 { return match ($v) { }; }",
    ] {
        assert!(from_watz(invalid).is_err(), "accepted {invalid}");
    }
    let deep = format!(
        "func $f(): i32 {{ return {}{{ 0 }}; }}",
        "if<i32> (1) { 0 } else ".repeat(512)
    );
    assert!(from_watz(&deep).is_err());
    for head in ["if", "match"] {
        let mut nested = "(nop)".to_owned();
        for _ in 0..32 {
            nested = if head == "if" {
                format!("(if (i32.const 1) (then {nested}) (else (nop) (nop)))")
            } else {
                format!("(@match $v (@default {nested}) (@case broken not-a-binding-list 0))")
            };
        }
        let source = format!("(module (@review-test {nested}))");
        let view = to_watz(&source).unwrap();
        assert!(same(
            &crate::sexpr::parse(&source).unwrap(),
            &crate::sexpr::parse(&from_watz(&view).unwrap().text).unwrap()
        ));
    }
}

#[test]
fn watz_case_keywords_lower_to_prefixed_walz_clauses() {
    let view = "func $read($v: $result<i64, i32>): i64 { return match ($v) { case ok($value) => $value, default => 0, }; }";
    let canonical = "(module (func $read (param $v (@inst $result i64 i32)) (result i64) (@match $v (@case ok ($value) $value) (@default 0))))";
    let lowered = from_watz(view).unwrap();
    assert!(same(
        &crate::sexpr::parse(canonical).unwrap(),
        &crate::sexpr::parse(&lowered.text).unwrap()
    ));
    let printed = to_watz(canonical).unwrap();
    assert!(printed.contains("case ok($value) =>"), "{printed}");
    assert!(printed.contains("default => 0"), "{printed}");
    assert!(
        !printed.contains("@case") && !printed.contains("@default"),
        "{printed}"
    );
    for clause in ["@case ok($n) => $n", "@default => 0"] {
        let source = format!("func $read(): i64 {{ return match ($v) {{ {clause} }}; }}");
        assert!(from_watz(&source).is_err(), "accepted {source}");
    }
}

#[test]
fn module_is_optional_and_let_keeps_the_canonical_binding_semantics() {
    let bare = "@export(\"main\") func $main(): i64 { let $a: i64 = 40, $b: i64 = 1; let $c: i64 = ($a + $b); return ($c + 1); }";
    let wrapped = format!("module {{ {bare} }}");
    let canonical = "(module (func $main (export \"main\") (result i64) (@let (($a (type i64) 40) ($b (type i64) 1)) (@let $c (type i64) (+ $a $b) (+ $c 1)))))";
    let expected = crate::sexpr::parse(canonical).unwrap();
    for source in [bare, &wrapped] {
        let lowered = from_watz(source).unwrap();
        assert!(same(
            &expected,
            &crate::sexpr::parse(&lowered.text).unwrap()
        ));
        assert_eq!(
            crate::compile(&lowered.text).unwrap().wasm,
            crate::compile(canonical).unwrap().wasm
        );
    }
    let printed = to_watz(canonical).unwrap();
    assert!(!printed.contains("module {"), "{printed}");
    assert!(
        printed.contains("let $a: i64 = 40, $b: i64 = 1;"),
        "{printed}"
    );
    assert!(printed.contains("let $c: i64"), "{printed}");
    assert!(!printed.contains("const "), "{printed}");
    for invalid in [
        "function $f(): i32 { return 1; }",
        "@wasmExport(\"main\") func $f(): i32 { return 1; }",
        "func $f(): i32 { const $x: i32 = 1; return $x; }",
        "func $f(): i32 { let $x: i32 = 1; $x = 2; return $x; }",
        "module { func $f(): i32 { return 1; }",
        "module { } func $f(): i32 { return 1; }",
        "}",
    ] {
        assert!(from_watz(invalid).is_err(), "accepted {invalid}");
    }
    for canonical in [
        "(module)",
        "(module $named (func (result i32) i32.const 42))",
    ] {
        let view = to_watz(canonical).unwrap();
        assert!(same(
            &crate::sexpr::parse(canonical).unwrap(),
            &crate::sexpr::parse(&from_watz(&view).unwrap().text).unwrap()
        ));
    }
}
