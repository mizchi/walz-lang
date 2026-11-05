#[test]
fn opaque_types_hide_representation_through_aliases_and_inferred_references() {
    let declarations = r#"(namespace $lib (public (opaque $State) (func $new) (func $get))
      (type $State (struct (field $n (mut i32))))
      (func $new (result (ref $State)) (struct.new $State (i32.const 42)))
      (func $get (param $s (ref $State)) (result i32) (struct.get $State $n (local.get $s))))"#;
    let valid = format!(
        "(module {declarations} (func $main (result i32) (call $lib.get (call $lib.new))))"
    );
    gcwat::compile(&valid).unwrap();
    for body in [
        "(struct.new $S (i32.const 0))",
        "(struct.get $S $n (call $lib.new))",
        "(struct.set $S $n (call $lib.new) (i32.const 0))",
    ] {
        let source = format!(
            "(module {declarations} (use $lib (type $State $S)) (func $bad (drop {body})))"
        );
        // Unit-producing struct.set needs no drop.
        let source = source.replace(
            "(drop (struct.set $S $n (call $lib.new) (i32.const 0)))",
            "(struct.set $S $n (call $lib.new) (i32.const 0))",
        );
        assert!(
            gcwat::check(&source)
                .unwrap_err()
                .to_string()
                .contains("opaque"),
            "{source}"
        );
    }
    let source = r#"(module (namespace $a (public (opaque $A) (func $new)) (type $A (array i32))
      (func $new (result (ref $A)) (array.new $A (i32.const 0) (i32.const 2))))
      (func $bad (result i32) (array.len (call $a.new))))"#;
    assert!(
        gcwat::check(source)
            .unwrap_err()
            .to_string()
            .contains("opaque")
    );
}
