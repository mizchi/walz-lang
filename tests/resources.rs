// Compiler scope fixtures use private expanded forms; public files use %use paths.
const SOURCE: &str = r#"(module
  (type $File (%resource "file" $close))
  (import "files" "open" (func $open (result i32)))
  (import "files" "read" (func $read (param $id i32) (result i32)))
  (import "files" "close" (func $close (param $id i32)))
  (func $read-file (param $f (%borrow $File)) (result i32) (%effect host reads)
    (call $read (%resource.handle (local.get $f))))
  (func $main (export "main") (result i32) (%effect host reads writes allocates)
    (%using $f (%resource.new $File (call $open)) (call $read-file (local.get $f)))))"#;

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
                "(%result i32) (%effect host reads writes allocates)",
                "(result (%borrow $File)) (%effect host reads writes allocates)",
            ),
        SOURCE.replace(
            "(call $read-file (local.get $f))",
            "(%resource.close (local.get $f))",
        ),
        SOURCE.replace(
            "(call $read-file (local.get $f))",
            "(%let $x (%resource.handle (local.get $f)) (local.get $x))",
        ),
        SOURCE.replace("(call $open)", "(i32.const 1)"),
        SOURCE
            .replace(
                "(call $read-file (local.get $f))",
                "(block (result i32) (drop (struct.new $Bad (local.get $f))) (i32.const 0))",
            )
            .replace(
                "(module",
                "(module (type $Bad (struct (field (%borrow $File))))",
            ),
    ] {
        assert!(
            wlt::check_with_policy(&source, &policy()).is_err(),
            "accepted: {source}"
        );
    }
}

#[test]
fn using_closes_before_recurring_outside_its_scope() {
    let source = SOURCE.replace(
        "(%using $f (%resource.new $File (call $open)) (call $read-file (local.get $f)))",
        "(%loop $again (result i32) (%using $f (%resource.new $File (call $open)) (%recur $again)))",
    );
    wlt::check_with_policy(&source, &policy()).unwrap();
    wlt::component::core_js::compile_with_policy(&source, &policy()).unwrap();
}

#[test]
fn resources_and_borrows_resolve_across_an_opaque_namespace() {
    let source = r#"(module
      (%legacy.namespace $file (%export (opaque $File) (func $new) (func $read))
        (type $File (%resource "file" $drop))
        (import "files" "open" (func $open (result i32)))
        (import "files" "read" (func $read-host (param $id i32) (result i32)))
        (import "files" "close" (func $drop (param $id i32)))
        (func $new (result (%resource $File)) (%resource.new $File (call $open)))
        (func $read (param $f (%borrow $File)) (result i32) (call $read-host (%resource.handle (local.get $f)))))
      (func $main (export "main") (result i32) (%effect host reads writes allocates)
        (%using $file (call $file.new) (call $file.read (local.get $file)))))"#;
    wlt::component::compile_with_policy(source, &policy()).unwrap();
}

#[test]
fn dce_retains_the_implicit_destructor_of_a_raw_resource_factory() {
    let source = r#"(module
      (import "files" "open" (func $open (result i32)))
      (import "files" "close" (func $close (param $id i32)))
      (func $main (export "main") (%effect host) (drop (call $open))))"#;
    let built = wlt::component::core_js::compile_with_policy(source, &policy()).unwrap();
    assert_eq!(built.manifest["imports"].as_array().unwrap().len(), 2);
}

#[test]
fn a_branch_cannot_carry_a_borrow_out_of_using() {
    let source=SOURCE.replace(
      "(%using $f (%resource.new $File (call $open)) (call $read-file (local.get $f)))",
      "(%let $escaped (block $done (result (%borrow $File)) (%using $f (%resource.new $File (call $open)) (br $done (local.get $f)))) (call $read-file (local.get $escaped)))");
    let error = wlt::check_with_policy(&source, &policy())
        .unwrap_err()
        .to_string();
    assert!(error.contains("borrow"), "{error}");
}

#[test]
fn a_trusted_reader_factory_returns_an_independent_affine_endpoint() {
    let source = r#"(module
      (type $Writer (%resource "channel" $close))
      (import "channels" "new" (func $new (result i32)))
      (import "channels" "reader" (func $reader (param $id i32) (result (%stream i32))))
      (import "channels" "close" (func $close (param $id i32)))
      (func $main (export "main") (%effect host reads writes allocates)
        (%using $w (%resource.new $Writer (call $new))
          (%let $r (call $reader (%resource.handle (local.get $w)))
            (%stream.close i32 (%move $r))))))"#;
    let text = r#"{"imports":[
      {"module":"channels","name":"new","params":[],"result":"i32","suspends":false,"capability":"channels","resource":{"kind":"channel","operation":"new"}},
      {"module":"channels","name":"reader","params":["i32"],"result":"(stream i32)","suspends":false,"capability":"channels","resource":{"kind":"channel","operation":"reader"}},
      {"module":"channels","name":"close","params":["i32"],"result":"unit","suspends":false,"capability":"channels","resource":{"kind":"channel","operation":"close"}}
    ]}"#;
    let policy = wlt::bindings::BindingPolicy::parse(text).unwrap();
    wlt::component::compile_with_policy(source, &policy).unwrap();
    wlt::component::core_js::compile_with_policy(source, &policy).unwrap();
    // Ordinary borrowed uses cannot produce owned results. A reader is an
    // explicit trusted factory, not a relaxation of the nonescaping borrow rule.
    let use_policy =
        wlt::bindings::BindingPolicy::parse(&text.replace("\"reader\"}}", "\"use\"}}")).unwrap();
    assert!(wlt::check_with_policy(source, &use_policy).is_err());
    assert!(
        wlt::check_with_policy(&source.replace("(%move $r)", "(local.get $r)"), &policy).is_err()
    );
}
