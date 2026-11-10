use serde_json::Value;

fn facts(source: &str, name: &str) -> Vec<Value> {
    let module = walz::check(source).unwrap();
    let report = serde_json::to_value(walz::analysis::analyze(&module)).unwrap();
    report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == name)
        .unwrap()["facts"]
        .as_array()
        .unwrap()
        .clone()
}
fn accesses(values: &[Value]) -> Vec<&Value> {
    values
        .iter()
        .filter(|f| f["operation"] == "array.get")
        .collect()
}

#[test]
fn unsigned_guard_proves_bounds_and_normal_array_length_proves_non_null() {
    let source = r#"(module (type $A (array (mut i32)))
      (func $get (param $a (ref null $A)) (param $i i32) (result i32)
        (@let $alias (local.get $a)
          (if (result i32) (i32.lt_u (local.get $i) (array.len (local.get $alias)))
            (then (array.get $A (local.get $a) (local.get $i))) (else (i32.const -1))))))"#;
    let f = facts(source, "$get");
    let a = accesses(&f);
    assert_eq!(a[0]["bounds_proven"], true);
    assert_eq!(a[0]["null_check_proven"], true);
}

#[test]
fn null_guards_refine_aliases_but_not_the_opposite_branch() {
    let source = r#"(module (type $C (struct (field i32)))
      (func $get (param $p (ref null $C)) (result i32)
        (@let $alias (local.get $p)
          (if (result i32) (ref.is_null (local.get $alias))
            (then (struct.get $C 0 (local.get $p)))
            (else (struct.get $C 0 (local.get $p)))))))"#;
    let f = facts(source, "$get");
    let reads = f
        .iter()
        .filter(|f| f["operation"] == "struct.get")
        .collect::<Vec<_>>();
    assert_eq!(reads[0]["null_check_proven"], false);
    assert_eq!(reads[1]["null_check_proven"], true);
}

#[test]
fn signed_guard_needs_nonnegative_index_and_distinct_arrays_are_not_equal() {
    let source = r#"(module (type $A (array (mut i32)))
      (func $bad (param $a (ref $A)) (param $i i32) (result i32)
        (if (result i32) (i32.lt_s (local.get $i) (array.len (local.get $a)))
          (then (array.get $A (local.get $a) (local.get $i))) (else (i32.const 0))))
      (func $good (param $a (ref $A)) (param $i i32) (result i32)
        (if (result i32) (i32.and (i32.ge_s (local.get $i) (i32.const 0)) (i32.lt_s (local.get $i) (array.len (local.get $a))))
          (then (array.get $A (local.get $a) (local.get $i))) (else (i32.const 0))))
      (func $different (param $a (ref $A)) (param $b (ref $A)) (param $i i32) (result i32)
        (if (result i32) (i32.lt_u (local.get $i) (array.len (local.get $a)))
          (then (array.get $A (local.get $b) (local.get $i))) (else (i32.const 0)))))"#;
    assert_eq!(accesses(&facts(source, "$bad"))[0]["bounds_proven"], false);
    assert_eq!(accesses(&facts(source, "$good"))[0]["bounds_proven"], true);
    assert_eq!(
        accesses(&facts(source, "$different"))[0]["bounds_proven"],
        false
    );
}

#[test]
fn fixed_lengths_and_wrapping_arithmetic_use_wasm_bits() {
    let source = r#"(module (type $A (array i32))
      (func $get (param $i i32) (result i32)
        (@let $a (array.new_fixed $A 3 (i32.const 1) (i32.const 2) (i32.const 3))
          (if (result i32) (i32.lt_u (local.get $i) (i32.const 2))
            (then (array.get $A (local.get $a) (i32.add (local.get $i) (i32.const 1))))
            (else (array.get $A (local.get $a) (i32.add (i32.const -1) (i32.const 1))))))))"#;
    let f = facts(source, "$get");
    let reads = accesses(&f);
    assert!(reads.iter().all(|f| f["bounds_proven"] == true));
    assert!(f.iter().any(|f| f["operation"] == "i32.add"
        && f["i32_range"]["min"] == 0
        && f["i32_range"]["max"] == 0));
}

#[test]
fn loop_carried_values_are_widened_and_only_normal_paths_refine_continuations() {
    let source = r#"(module (type $A (array i32))
      (func $loop (param $n i32) (result i32)
        (@let $a (array.new_fixed $A 1 (i32.const 42))
        (@loop $go (bind $i i32 (i32.const 0)) (result i32)
          (if (result i32) (i32.eqz (local.get $n))
            (then (array.get $A (local.get $a) (local.get $i)))
            (else (@recur $go (i32.const -1)))))))
      (func $guard (param $a (ref null $A)) (result i32)
        (block (result i32)
          (if (ref.is_null (local.get $a)) (then (unreachable)) (else (nop)))
          (array.len (local.get $a)))))"#;
    assert_eq!(accesses(&facts(source, "$loop"))[0]["bounds_proven"], false);
    assert!(
        facts(source, "$guard")
            .iter()
            .any(|f| f["operation"] == "array.len" && f["null_check_proven"] == true)
    );
}

#[test]
fn allocation_length_snapshots_prove_map_output_bounds_without_aliasing_the_arrays() {
    let source = include_str!("../examples/higher-order.walz");
    let f = facts(source, "$map");
    assert!(
        f.iter()
            .filter(|f| f["operation"] == "array.get" || f["operation"] == "array.set")
            .all(|f| f["bounds_proven"] == true)
    );
}

#[test]
fn inferred_ranges_contain_concrete_wrapping_results_at_boundary_constants() {
    let operations = [
        "add", "sub", "mul", "and", "or", "xor", "shl", "shr_u", "shr_s",
    ];
    let constants = [0u32, 1, 31, 32, 0x7fffffff, 0x80000000, u32::MAX];
    let mut source = String::from("(module");
    for (op_index, op) in operations.iter().enumerate() {
        for (c_index, c) in constants.iter().enumerate() {
            source.push_str(&format!("(func $f{op_index}_{c_index} (param $x i32) (result i32) (if (result i32) (i32.lt_u (local.get $x) (i32.const 16)) (then (i32.{op} (local.get $x) (i32.const {c}))) (else (i32.const 0))))"));
        }
    }
    source.push(')');
    let module = walz::check(&source).unwrap();
    let report = serde_json::to_value(walz::analysis::analyze(&module)).unwrap();
    for (op_index, op) in operations.iter().enumerate() {
        for (c_index, &c) in constants.iter().enumerate() {
            let f = &report["functions"][op_index * constants.len() + c_index];
            let fact = f["facts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["operation"] == format!("i32.{op}"))
                .unwrap();
            let min = fact["i32_range"]["min"].as_u64().unwrap() as u32;
            let max = fact["i32_range"]["max"].as_u64().unwrap() as u32;
            for x in 0u32..16 {
                let result = match *op {
                    "add" => x.wrapping_add(c),
                    "sub" => x.wrapping_sub(c),
                    "mul" => x.wrapping_mul(c),
                    "and" => x & c,
                    "or" => x | c,
                    "xor" => x ^ c,
                    "shl" => x.wrapping_shl(c & 31),
                    "shr_u" => x >> (c & 31),
                    "shr_s" => ((x as i32) >> (c & 31)) as u32,
                    _ => unreachable!(),
                };
                assert!(
                    (min..=max).contains(&result),
                    "{op}({x},{c})={result} outside {min}..{max}"
                );
            }
        }
    }
}

#[test]
fn signed_negative_index_counterexample_is_a_real_wasm_trap() {
    use std::process::Command;
    let source = r#"(module (type $A (array i32))
      (func $main (export "main") (result i32)
        (@let $a (array.new_fixed $A 3 (i32.const 1) (i32.const 2) (i32.const 3))
          (if (result i32) (i32.lt_s (i32.const -1) (array.len (local.get $a)))
            (then (array.get $A (local.get $a) (i32.const -1))) (else (i32.const 0))))))"#;
    let compiled = walz::compile(source).unwrap();
    let report = serde_json::to_value(&compiled.analysis).unwrap();
    let access = report["functions"][0]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["operation"] == "array.get")
        .unwrap();
    assert_eq!(access["bounds_proven"], false);
    assert_eq!(access["normal_return_possible"], false);
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("counterexample.wasm");
    std::fs::write(&binary, compiled.wasm).unwrap();
    let output = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            "main",
        ])
        .arg(binary)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("out of bounds"));
}

#[test]
fn labeled_branch_results_do_not_inherit_facts_from_the_normal_tail() {
    let source = r#"(module (type $A (array i32))
      (func $f (param $choose i32) (result i32)
        (@let $a (array.new_fixed $A 1 (i32.const 42))
          (@let $i (block $done (result i32)
            (if (local.get $choose) (then (br $done (i32.const 99))) (else (nop)))
            (i32.const 0))
            (array.get $A (local.get $a) (local.get $i))))))"#;
    assert_eq!(accesses(&facts(source, "$f"))[0]["bounds_proven"], false);
}
