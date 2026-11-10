// Compiler scope fixtures use private expanded forms; public files use @use paths.
fn rejects(source: &str, message: &str) {
    let error = walz::check(source).unwrap_err().to_string();
    assert!(error.contains(message), "{source}\n{error}");
}

#[test]
fn aliases_are_transparent_and_forward_references_share_value_types() {
    let source = r#"(module
      (@type.alias $Scan (@tuple $State i32))
      (@type.alias $State (ref $Cell))
      (type $Cell (struct (field i32)))
      (func $read (param $scan $Scan) (result i32)
        (@match $Scan (result i32) (local.get $scan)
          (@case pair ($state $n)
            (i32.add (struct.get $Cell 0 (local.get $state)) (local.get $n)))))
      (func $main (result i32)
        (call $read (@tuple.new $State i32 (struct.new $Cell (i32.const 40)) (i32.const 2)))))"#;
    let module = walz::check(source).unwrap();
    assert_eq!(
        module.type_name(&module.functions[0].locals[0].ty),
        "(@tuple (ref $Cell) i32)"
    );
    assert!(
        module
            .types
            .iter()
            .all(|ty| ty.name != "$Scan" && ty.name != "$State")
    );
    walz::compile(source).unwrap();
}

#[test]
fn aliases_cover_storage_signatures_captures_and_tag_payloads() {
    walz::compile(
        r#"(module
      (@type.alias $Value (@option i32))
      (type $Box (struct (field $value (mut $Value))))
      (type $Values (array $Value))
      (type $Choice (@variant (value $Value)))
      (tag $Failure (param $Value))
      (import "host" "read" (func $read (param $value $Value) (result $Value)))
      (@closure $Get (@capture $value $Value) (param $ignored $Value) (result $Value)
        (local.get $value))
      (func $f (param $value $Value) (result $Value)
        (@let $copy $Value (local.get $value)
          (@closure.call $Get (@closure.new $Get (local.get $copy)) (local.get $value)))))"#,
    )
    .unwrap();
}

#[test]
fn aliases_cover_loop_bindings_results_and_value_constructor_arguments() {
    walz::compile(
        r#"(module
      (@type.alias $N i32)
      (@type.alias $O (@option $N))
      (@type.alias $R (@result $N $N))
      (func $loop (param $value $N) (result $O)
        (@loop $again (bind $n $N (local.get $value)) (result $O)
          (if (result $O) (i32.eqz (local.get $n))
            (then (@option.none $N))
            (else (@option.some $N (local.get $n))))))
      (func $for (param $value $N)
        (@for $again (bind $n $N (local.get $value))
          (while (i32.const 0)) (step (local.get $n)) (nop)))
      (func $result (result $R) (@result.ok $N $N (i32.const 42)))
      (func $box (@box.close (@box.new $N (i32.const 1))))
      (func $future (@future.close $N (@future.ready $N (i32.const 1))))
      (func $stream (@stream.close $N (@stream.cons $N (i32.const 1) (@stream.empty $N)))))"#,
    )
    .unwrap();
}

#[test]
fn aliases_work_in_explicit_scoped_forms_and_inferred_constructors() {
    walz::compile(
        r#"(module
      (@type.alias $N i64)
      (@type.alias $E i32)
      (@type.alias $R (@result $N $E))
      (@type.alias $P (@tuple $N $E))
      (func $map (param $input $R) (result $R)
        (@result.let ($value) $R $N (local.get $input)
          (@result.ok $N $E (local.get $value))))
      (func $fold (param $source (@stream $R)) (result $R)
        (@stream.fold-result $N $N $E (@move $source) (i64.const 0) ($state $item)
          (@result.ok $N $E (i64.add (local.get $state) (local.get $item)))))
      (func $inferred (result $R) (@result.ok (i64.const 42)))
      (func $tuple (result $P)
        (@let ($left $right) (@tuple.new $N $E (i64.const 40) (i32.const 2))
          (@tuple.new (local.get $left) (local.get $right)))))"#,
    )
    .unwrap();
}

#[test]
fn unused_aliases_are_checked_without_changing_executable_types() {
    let plain =
        walz::compile("(module (func $f (export \"f\") (result i32) (i32.const 42)))").unwrap();
    let aliased = walz::compile(
        "(module (@type.alias $Unused (@option i64)) (func $f (export \"f\") (result i32) (i32.const 42)))",
    )
    .unwrap();
    assert_eq!(plain.wasm, aliased.wasm);
    rejects(
        "(module (@type.alias $Invalid (@box (@box i32))))",
        "own payload must be shared",
    );
    rejects(
        "(module (@type.alias $Invalid (@future (@box i32))))",
        "delivery payload must be shared",
    );
}

#[test]
fn aliases_do_not_rewrite_names_in_other_namespaces() {
    let source = r#"(module
      (@type.alias $Value (@option i32))
      (func $Value (param $Value $Value) (result $Value)
        (@let $Value $Value (local.get $Value) (local.get $Value)))
      (type $Choice (@variant (Value $Value)))
      (func $f (result $Value) (call $Value (@option.none i32))))"#;
    let module = walz::check(source).unwrap();
    assert_eq!(module.functions[0].name, "$Value");
    assert_eq!(module.functions[0].locals[0].name, "$Value");
    walz::compile(source).unwrap();
}

#[test]
fn aliases_preserve_ownership_rules() {
    walz::compile(
        r#"(module
      (@type.alias $Owner (@box i64))
      (@type.alias $Wrapped (@option $Owner))
      (func $take (param $x $Wrapped) (result i64)
        (@match $Wrapped (result i64) (@move $x)
          (@case none () (i64.const 0))
          (@case some ($owner)
            (@match $Owner (result i64) (@move $owner)
              (@case own ($n) (local.get $n)))))))"#,
    )
    .unwrap();
    rejects(
        "(module (@type.alias $Owner (@box i64)) (func $bad (param $x $Owner) (@box.close (local.get $x))))",
        "explicit move",
    );
    rejects(
        "(module (@type.alias $Owner (@box i64)) (type $Box (struct (field $Owner))))",
        "shared GC storage",
    );
}

#[test]
fn unit_aliases_are_only_accepted_as_constructed_payloads() {
    walz::compile("(module (@type.alias $Nothing @unit) (func $f (result (@result $Nothing i32)) (@result.ok $Nothing i32)))").unwrap();
    rejects(
        "(module (@type.alias $Nothing @unit) (func $f (result $Nothing) (nop)))",
        "expected numeric type",
    );
}

#[test]
fn aliases_follow_namespace_visibility_and_use_renaming() {
    walz::compile(
        r#"(module
      (@file.use $lib (type $Scan $Imported))
      (@legacy.namespace $lib
        (@public (type $Scan) (func $new))
        (@type.alias $Scan (@tuple (ref $State) i32))
        (type $State (struct))
        (func $new (result $Scan) (@tuple.new (ref $State) i32 (struct.new $State) (i32.const 42))))
      (func $read (param $scan $Imported) (result i32)
        (@match $Imported (result i32) (local.get $scan)
          (@case pair ($state $n) (local.get $n))))
      (func $main (result i32) (call $read (call $lib.new))))"#,
    )
    .unwrap();
    rejects(
        "(module (@legacy.namespace $lib (@type.alias $Private i32)) (func $f (param $x $lib.Private) (nop)))",
        "private",
    );
}

#[test]
fn aliases_cannot_expose_opaque_representation() {
    let declarations = r#"(@legacy.namespace $lib
      (@public (opaque $State) (type $Reference) (func $new))
      (type $State (struct (field i32)))
      (@type.alias $Reference (ref $State))
      (func $new (result $Reference) (struct.new $State (i32.const 42))))"#;
    walz::compile(&format!(
        "(module {declarations} (func $f (result $lib.Reference) (call $lib.new)))"
    ))
    .unwrap();
    rejects(
        &format!(
            "(module {declarations} (func $f (result i32) (@let $state $lib.Reference (call $lib.new) (struct.get $lib.State 0 (local.get $state)))))"
        ),
        "opaque",
    );
    rejects(
        "(module (@legacy.namespace $lib (@public (opaque $Alias)) (@type.alias $Alias i32)))",
        "opaque",
    );
}

#[test]
fn malformed_cyclic_duplicate_and_heap_aliases_are_rejected() {
    for source in [
        "(module (@type.alias))",
        "(module (@type.alias $A))",
        "(module (@type.alias $A i32 i64))",
        "(module (@type.alias nope i32))",
        "(module (@type.alias $A (@option)))",
        "(module (@type.alias $A (i32.const 0)))",
        "(module (@type.alias $A $Missing))",
        "(module (@type.alias $A (ref $Missing)))",
        "(module (@type.alias $A (@option $A)))",
        "(module (@type.alias $A $B) (@type.alias $B $A))",
        "(module (@type.alias $A i32) (@type.alias $A i64))",
        "(module (@type.alias $A i32) (type $A (struct)))",
        "(module (@type.alias $A i32) (@closure $A (nop)))",
        "(module (@type.alias $A (@option i32)) (func $f (param $x (ref $A)) (nop)))",
        "(module (@type.alias $A (ref $B)) (type $B (struct)) (func $f (result i32) (struct.get $A 0 (struct.new $B))))",
    ] {
        assert!(walz::check(source).is_err(), "{source}");
    }
}

#[test]
fn recursive_expansion_is_bounded_before_allocating_large_types() {
    let mut source = String::from("(module (@type.alias $A0 i32)");
    for index in 1..25 {
        source.push_str(&format!(
            "(@type.alias $A{index} (@tuple $A{} $A{}))",
            index - 1,
            index - 1
        ));
    }
    source.push(')');
    rejects(&source, "limit");
    let mut source = String::from("(module (@type.alias $A0 i32)");
    for index in 1..300 {
        source.push_str(&format!(
            "(@type.alias $A{index} (@option $A{}))",
            index - 1
        ));
    }
    source.push(')');
    rejects(&source, "limit");
}
