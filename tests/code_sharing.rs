use walz::code_sharing::inventory;

fn inspect(source: &str) -> walz::code_sharing::Inventory {
    inventory(&wat::parse_str(source).unwrap()).unwrap()
}

#[test]
fn groups_exact_bodies_without_confusing_exports_types_or_dependencies() {
    let report = inspect(
        r#"(module
          (func $a (result i32) i32.const 42)
          (func $b (result i32) i32.const 42)
          (func $public (export "public") (result i32) i32.const 42)
          (func $c (param i32) (result i32) i32.const 42)
          (func $d (result i32) call $a)
          (func $e (result i32) call $b))"#,
    );
    assert_eq!(report.functions.len(), 6);
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].members, [0, 1, 2]);
    assert_eq!(report.groups[0].eligible, [0, 1]);
    assert_eq!(report.functions[4].calls, [0]);
    assert_eq!(report.functions[5].calls, [1]);
    assert!(report.code_section_bytes > report.functions.iter().map(|f| f.body_bytes).sum());
    assert!(report.type_section_bytes > 0);
}

#[test]
fn references_from_elements_globals_and_instructions_protect_identity() {
    let report = inspect(
        r#"(module
          (func $a (result i32) i32.const 42)
          (func $b (result i32) i32.const 42)
          (func $c (result i32) i32.const 42)
          (elem declare func $a $b $c)
          (global funcref (ref.func $b))
          (func (export "get") (result funcref) ref.func $c))"#,
    );
    assert!(report.functions[..3].iter().all(|f| f.address_taken));
    assert!(report.groups[0].eligible.is_empty());
}

#[test]
fn gc_type_immediates_and_numeric_bits_are_part_of_the_key() {
    let report = inspect(
        r#"(module
          (type $a (struct)) (type $b (struct))
          (func (result (ref $a)) struct.new $a)
          (func (result (ref $b)) struct.new $b)
          (func (result f64) f64.const 0)
          (func (result f64) f64.const -0))"#,
    );
    assert_eq!(report.gc_types, 2);
    assert!(report.groups.is_empty());
    assert_eq!(report.functions[0].name, None);
    assert_eq!(report.functions[0].instructions, 2);
}

#[test]
fn declared_function_subtypes_remain_distinct_candidates() {
    let report = inspect(
        r#"(module
      (type $parent (sub (func (result i32))))
      (type $child (sub $parent (func (result i32))))
      (func (type $parent) i32.const 42)
      (func (type $child) i32.const 42))"#,
    );
    assert!(report.groups.is_empty());
}

#[test]
fn imports_keep_their_index_space_and_invalid_modules_are_rejected() {
    let report = inspect(
        r#"(module (import "host" "f" (func $host (result i32)))
          (func $first (result i32) call $host)
          (func $second (result i32) call $host))"#,
    );
    assert_eq!(report.imported_functions, 1);
    assert_eq!(report.groups[0].members, [1, 2]);
    assert_eq!(report.functions[0].name.as_deref(), Some("first"));
    assert!(inventory(&wat::parse_str("(module (func (result i32)))").unwrap()).is_err());
}

#[test]
fn exact_sharing_remaps_calls_and_names_but_preserves_public_identity() {
    let input = wat::parse_str(
        r#"(module
      (func $a (result i32) i32.const 42)
      (func $b (result i32) i32.const 42)
      (func $public (export "public") (result i32) i32.const 42)
      (func $entry (export "f") (result i32) call $a call $b i32.add))"#,
    )
    .unwrap();
    let output = walz::code_sharing::share_exact(&input).unwrap();
    let report = inventory(&output).unwrap();
    assert_eq!(report.functions.len(), 3);
    assert_eq!(report.functions[0].name.as_deref(), Some("a"));
    assert_eq!(report.functions[1].exports, ["public"]);
    assert_eq!(report.functions[2].calls, [0]);
    assert!(output.len() < input.len());
    assert_eq!(walz::code_sharing::share_exact(&output).unwrap(), output);
}

#[test]
fn exact_sharing_keeps_taken_functions_and_refuses_unknown_index_metadata() {
    let input = wat::parse_str(
        r#"(module
      (func $a (result i32) i32.const 42)
      (func $b (result i32) i32.const 42)
      (elem declare func $a $b))"#,
    )
    .unwrap();
    assert_eq!(walz::code_sharing::share_exact(&input).unwrap(), input);
    let input = wat::parse_str(
        r#"(module
      (@custom "index-metadata" "opaque")
      (func (result i32) i32.const 42)
      (func (result i32) i32.const 42))"#,
    )
    .unwrap();
    assert!(walz::code_sharing::share_exact(&input).is_err());
}

#[test]
fn remaps_import_prefix_elements_globals_and_ref_func_after_removal() {
    let input = wat::parse_str(
        r#"(module
      (import "host" "f" (func $host (result i32)))
      (func $a (result i32) call $host)
      (func $b (result i32) call $host)
      (func $x (result i32) i32.const 7)
      (func $y (result i32) i32.const 7)
      (table 2 funcref) (elem (i32.const 0) func $x $y)
      (global funcref (ref.func $y))
      (func $entry (export "f") (result i32) call $b)
      (func $get (export "get") (result funcref) ref.func $y))"#,
    )
    .unwrap();
    let output = walz::code_sharing::share_exact(&input).unwrap();
    let report = inventory(&output).unwrap();
    assert_eq!(report.imported_functions, 1);
    assert_eq!(report.functions.len(), 5);
    assert_eq!(report.functions[0].calls, [0]);
    assert_eq!(report.functions[3].calls, [1]);
    assert!(report.functions[1].address_taken && report.functions[2].address_taken);
    for payload in wasmparser::Parser::new(0).parse_all(&output) {
        match payload.unwrap() {
            wasmparser::Payload::ElementSection(reader) => {
                for e in reader {
                    let wasmparser::ElementItems::Functions(items) = e.unwrap().items else {
                        panic!()
                    };
                    assert_eq!(
                        items.into_iter().collect::<Result<Vec<_>, _>>().unwrap(),
                        [2, 3]
                    );
                }
            }
            wasmparser::Payload::GlobalSection(reader) => {
                for g in reader {
                    assert!(matches!(
                        g.unwrap().init_expr.get_operators_reader().read().unwrap(),
                        wasmparser::Operator::RefFunc { function_index: 3 }
                    ));
                }
            }
            _ => (),
        }
    }
}
