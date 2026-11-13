#[path = "../tools/selfhost/gc_contract.rs"]
mod gc_contract;

fn canonical(source: &str) -> Vec<u8> {
    gc_contract::canonical(&wat::parse_str(source).unwrap()).unwrap()
}

#[test]
fn gc_parity_normalizes_only_the_function_pool_and_debug_names() {
    let a = canonical(
        r#"(module
      (type $S (struct (field i32)))
      (func $a (export "a") (param $s (ref null $S)) (result i32) (struct.get $S 0 (local.get $s)))
      (func $b (export "b") (param $s (ref null $S)) (result i32) (struct.get $S 0 (local.get $s))))"#,
    );
    let b = canonical(
        r#"(module
      (type $S (struct (field i32)))
      (type $F (func (param (ref null $S)) (result i32)))
      (type $G (func (param (ref null $S)) (result i32)))
      (func (export "a") (type $F) (struct.get $S 0 (local.get 0)))
      (func (export "b") (type $G) (struct.get $S 0 (local.get 0))))"#,
    );
    assert_eq!(a, b);
}

#[test]
fn gc_parity_preserves_recursion_groups_mutability_and_dead_code() {
    assert_ne!(
        canonical("(module (type $S (struct (field i32))))"),
        canonical("(module (rec (type $S (struct (field i32)))))")
    );
    assert_ne!(
        canonical("(module (type $S (struct (field i32))))"),
        canonical("(module (type $S (struct (field (mut i32)))))")
    );
    assert_ne!(
        canonical("(module (type $S (struct (field (ref null $S)))))"),
        canonical("(module (type $S (struct (field (ref null any)))))")
    );
    assert_ne!(
        canonical("(module (func (unreachable) (drop (ref.i31 (i32.const 1)))))"),
        canonical("(module (func (unreachable) (drop (ref.i31 (i32.const 2)))))")
    );
}

#[test]
fn gc_parity_fails_closed_outside_the_checked_subset() {
    for source in [
        "(module (memory 1))",
        "(module (type $F (func)) (func (export \"f\") (result funcref) (ref.null func)))",
        "(module (type $S (struct (field f32))))",
        "(module (func (result i32) (i32.clz (i32.const 1))))",
        "(module (func (result anyref) (any.convert_extern (unreachable))))",
    ] {
        assert!(
            gc_contract::canonical(&wat::parse_str(source).unwrap()).is_err(),
            "{source}"
        );
    }
}
