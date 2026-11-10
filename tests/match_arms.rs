use std::process::Command;

fn run(source: &str) -> String {
    let built = walz::compile(source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("match.wasm");
    std::fs::write(&path, built.wasm).unwrap();
    let output = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y",
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
    String::from_utf8(output.stdout).unwrap().trim().into()
}

#[test]
fn case_and_default_cover_remaining_variants_in_both_match_forms() {
    for expression in [
        "(@match $value (@case a ($n) $n) (@default 42))",
        "(@match $T (result i32) $value (@case a ($n) $n) (@default 42))",
        "(@match $value (@default 42))",
    ] {
        let source = format!(
            "(module (type $T (@variant (a i32) (b i64) (empty))) (func $read (param $value (ref $T)) (result i32) {expression}) (func $main (export \"main\") (result i32) (call $read (@variant.new $T b (i64.const 9)))))"
        );
        assert_eq!(run(&source), "42");
    }
}

#[test]
fn defaults_evaluate_the_subject_and_selected_body_once() {
    let source = r#"(module
      (type $T (@variant (a) (b) (c)))
      (type $Counter (struct (field $n (mut i32))))
      (func $subject (param $counter (ref $Counter)) (result (ref $T))
        (block (result (ref $T))
          (struct.set $Counter $n $counter (+ (struct.get $Counter $n $counter) 1))
          (@variant.new $T c)))
      (func $main (export "main") (result i32)
        (@let $counter (struct.new $Counter 0)
          (+ (@match (call $subject $counter)
               (@case a () 0)
               (@default (block (result i32)
                 (struct.set $Counter $n $counter (+ (struct.get $Counter $n $counter) 1)) 40)))
             (struct.get $Counter $n $counter)))))"#;
    assert_eq!(run(source), "42");
}

#[test]
fn defaults_are_unique_final_and_cover_at_least_one_case() {
    for arms in [
        "(@default 0) (@default 1)",
        "(@default 0) (@case a () 1)",
        "(@case a () 0) (@case b () 1) (@default 2)",
        "(@default () 0)",
    ] {
        let source = format!(
            "(module (type $T (@variant (a) (b))) (func $f (param $v (ref $T)) (result i32) (@match $v {arms})))"
        );
        let error = walz::check(&source).unwrap_err();
        assert_eq!(
            error.diagnostic().code,
            "E_MATCH_DEFAULT",
            "{source}: {error}"
        );
    }
}

#[test]
fn default_expansion_has_a_finite_work_budget() {
    let variants = (0..512).map(|i| format!("(v{i})")).collect::<String>();
    let body = format!("(block (result i32) {} 42)", "(nop) ".repeat(255));
    let source = format!(
        "(module (type $E (@variant {variants})) (func $f (param $x (ref $E)) (result i32) (@match $x (@default {body}))))"
    );
    let error = match walz::check(&source) {
        Err(error) => error,
        Ok(_) => panic!("oversized default expansion was accepted"),
    };
    assert_eq!(error.diagnostic().code, "E_MATCH_DEFAULT");
    assert!(error.to_string().contains("expansion limit"), "{error}");
}

#[test]
fn default_preserves_type_effect_scope_and_ownership_checks() {
    for source in [
        "(module (@use \"walz:core\" (type $option)) (func $f (param $v (@inst $option i32)) (result i32) (@match $v (@default (i64.const 0)))))",
        "(module (@use \"walz:core\" (type $option)) (func $f (param $v (@inst $option i32)) (result i32) (@match $v (@default $missing))))",
        "(module (@use \"walz:core\" (type $option)) (import \"host\" \"work\" (func $work (result i32))) (func $f (param $v (@inst $option i32)) (result i32) (@effect pure) (@match $v (@default (call $work)))))",
        "(module (@use \"walz:core\" (type $option) (type $box)) (func $f (param $v (@inst $option i32)) (@let $owner (@box.new i32 1) (@match $v (@case some ($n) (@box.close (@move $owner))) (@default (nop))))))",
    ] {
        assert!(walz::compile(source).is_err(), "accepted {source}");
    }
    let source = "(module (@use \"walz:core\" (type $result) (type $box)) (func $f (param $v (@inst $result (@inst $box i32) i32)) (result i32) (@match (@move $v) (@default 0))))";
    let error = walz::check(source).unwrap_err();
    assert_eq!(error.diagnostic().code, "E_MATCH_DEFAULT", "{error}");
}

#[test]
fn unprefixed_arms_request_the_explicit_spelling() {
    for (arm, spelling) in [
        ("(case a () 0) (@case b () 1)", "@case"),
        ("(default 0)", "@default"),
    ] {
        let source = format!(
            "(module (type $T (@variant (a) (b))) (func $f (param $v (ref $T)) (result i32) (@match $v {arm})))"
        );
        let error = walz::check(&source).unwrap_err();
        assert_eq!(error.diagnostic().code, "E_WALZ_PREFIX", "{error}");
        assert!(error.to_string().contains(spelling), "{error}");
    }
}

#[test]
fn default_macro_bodies_keep_local_scope_and_owned_results() {
    let source = r#"(module
      (@use "walz:core" (type $box))
      (type $T (@variant (a) (b) (c)))
      (@macro $fallback (rule () (@let $n (type i32) 40 (@box.new i32 (+ $n 2)))))
      (func $read (param $v (ref $T)) (result (@inst $box i32))
        (@match $v (@case a () (@box.new i32 0)) (@default (@$fallback))))
      (func $main (export "main") (result i32)
        (@match (call $read (@variant.new $T c)) (@case own ($value) $value))))"#;
    assert_eq!(run(source), "42");
}
