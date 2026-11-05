const SOURCE: &str = r#"(module
  (type $File (resource "file" $close))
  (import "files" "open" (func $open (result i32)))
  (import "files" "read" (func $read (param $id i32) (result i32)))
  (import "files" "close" (func $close (param $id i32)))
  (func $read-file (param $f (borrow $File)) (result i32) (effects host reads)
    (call $read (resource.handle (local.get $f))))
  (func $main (export "main") (result i32) (effects host reads writes allocates)
    (using $f (resource.new $File (call $open)) (call $read-file (local.get $f)))))"#;

fn policy() -> wlt::bindings::BindingPolicy {
    wlt::bindings::BindingPolicy::parse(r#"{"imports":[
      {"module":"files","name":"open","params":[],"result":"i32","suspends":false,"capability":"file","resource":{"kind":"file","operation":"new"}},
      {"module":"files","name":"read","params":["i32"],"result":"i32","suspends":false,"capability":"file","resource":{"kind":"file","operation":"use"}},
      {"module":"files","name":"close","params":["i32"],"result":"unit","suspends":false,"capability":"file","resource":{"kind":"file","operation":"close"}}
    ]}"#).unwrap()
}

#[test]
fn resource_using_is_typed_and_independently_checks_ownership() {
    let p = policy();
    let module = wlt::check_with_policy(SOURCE, &p).unwrap();
    wlt::ownership::validate(SOURCE, &module).unwrap();
    wlt::component::compile_with_policy(SOURCE, &p).unwrap();
    wlt::component::core_js::compile_with_policy(SOURCE, &p).unwrap();
}

#[test]
fn borrowed_resources_cannot_escape_or_be_duplicated() {
    for source in [
        SOURCE
            .replace("(call $read-file (local.get $f))", "(local.get $f)")
            .replace(
                "(result i32) (effects host reads writes allocates)",
                "(result (borrow $File)) (effects host reads writes allocates)",
            ),
        SOURCE.replace(
            "(call $read-file (local.get $f))",
            "(resource.close (local.get $f))",
        ),
        SOURCE.replace(
            "(call $read-file (local.get $f))",
            "(let $x (resource.handle (local.get $f)) (local.get $x))",
        ),
        SOURCE.replace("(call $open)", "(i32.const 1)"),
        SOURCE
            .replace(
                "(call $read-file (local.get $f))",
                "(block (result i32) (drop (struct.new $Bad (local.get $f))) (i32.const 0))",
            )
            .replace(
                "(module",
                "(module (type $Bad (struct (field (borrow $File))))",
            ),
    ] {
        assert!(
            wlt::check_with_policy(&source, &policy()).is_err(),
            "accepted: {source}"
        );
    }
}

#[test]
fn using_rejects_a_branch_that_skips_its_destructor() {
    let source = SOURCE.replace(
        "(using $f (resource.new $File (call $open)) (call $read-file (local.get $f)))",
        "(loop $again (result i32) (using $f (resource.new $File (call $open)) (recur $again)))",
    );
    assert!(
        wlt::check_with_policy(&source, &policy())
            .unwrap_err()
            .to_string()
            .contains("using scope")
    );
}
