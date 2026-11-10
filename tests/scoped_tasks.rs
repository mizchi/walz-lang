const SOURCE: &str = r#"(module
  (@closure $Work (@capture $n i32) (result i32) (@effect pure) (+ $n 1))
  (func $main (export "main") (result i32) (@effect host reads writes allocates suspends)
    (@task.scope (result i32)
      (@let $a (@task.spawn i32 $Work (@closure.new $Work 20))
        (@let $b (@task.spawn i32 $Work (@closure.new $Work 20))
          (+ (@task.join i32 (@move $a)) (@task.join i32 (@move $b))))))))"#;

#[test]
fn scoped_spawn_checks_closures_results_ownership_and_both_backends() {
    walz::check(SOURCE).unwrap();
    walz::component::core_js::compile(SOURCE).unwrap();
    walz::component::compile(SOURCE).unwrap();
    for source in [
        SOURCE.replace("(@move $a)", "$a"),
        SOURCE.replace("@task.spawn i32 $Work", "@task.spawn i64 $Work"),
        SOURCE.replace(
            "(result i32) (@effect host reads writes allocates suspends)",
            "(result i32) (@effect pure)",
        ),
        SOURCE.replace("(@task.join i32 (@move $b))", "(i32.const 21)"),
    ] {
        assert!(walz::check(&source).is_err(), "accepted {source}");
    }
}

#[test]
fn task_handles_cannot_cross_the_portable_abi_or_be_forged_as_variants() {
    let escaping = r#"(module (@closure $C (result i32) (@effect pure) 42)
      (func $main (export "main") (result (@task i32)) (@effect host reads writes allocates suspends)
        (@task.spawn i32 $C (@closure.new $C))))"#;
    assert!(walz::component::compile(escaping).is_err());
    assert!(walz::component::core_js::compile(escaping).is_err());
    let forging = r#"(module (func $fake (result (@task i32)) (@variant.new (@task i32) task 1)))"#;
    assert!(walz::check(forging).is_err());
}

#[test]
fn extension_names_remain_available_as_enum_case_labels() {
    let source = r#"(module (type $Event (@variant (task i32) (future i32) (stream i32)))
      (func $main (export "main") (result i32) (@effect allocates)
        (@match $Event (result i32) (@variant.new $Event task 42)
          (@case task ($n) $n) (@case future ($n) $n) (@case stream ($n) $n))))"#;
    walz::compile(source).unwrap();
}

#[test]
fn nominal_task_results_work_after_representation_erasure() {
    let source = r#"(module
      (type $N (@newtype i32))
      (@closure $A (result i32) (@effect pure) 20)
      (@closure $B (result $N) (@effect pure) (@newtype.wrap $N 22))
      (func $main (export "main") (result i32) (@effect host reads writes allocates suspends)
        (@let $a (@task.spawn i32 $A (@closure.new $A))
          (@let $b (@task.spawn $N $B (@closure.new $B))
            (+ (@task.join i32 (@move $a)) (@newtype.unwrap $N (@task.join $N (@move $b))))))))"#;
    walz::component::core_js::compile(source).unwrap();
    walz::component::compile(source).unwrap();
}
