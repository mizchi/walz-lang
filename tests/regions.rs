use serde_json::Value;

fn memory(source: &str, name: &str) -> Value {
    let module = wlt::check(source).unwrap();
    let report = serde_json::to_value(wlt::analysis::analyze(&module)).unwrap();
    report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == name)
        .unwrap()["memory"]
        .clone()
}

#[test]
fn distinguishes_input_mutation_from_fresh_output_and_return_escape() {
    let source = r#"(module
      (type $A (array (mut i32)))
      (func $map (param $input (ref $A)) (result (ref $A))
        (%let $out (array.new $A (i32.const 0) (array.len (local.get $input)))
          (block (result (ref $A))
            (array.set $A (local.get $out) (i32.const 0) (array.get $A (local.get $input) (i32.const 0)))
            (local.get $out))))
      (func $sort (param $input (ref $A)) (array.set $A (local.get $input) (i32.const 0) (i32.const 42))))"#;
    let map = memory(source, "$map");
    assert_eq!(map["reads"][0]["object"]["root"]["kind"], "parameter");
    assert_eq!(map["writes"][0]["object"]["root"]["kind"], "allocation");
    assert_eq!(map["allocations"][0]["escapes"]["returned"], true);
    assert_eq!(map["allocations"][0]["escapes"]["stored"], false);
    assert_eq!(
        memory(source, "$sort")["writes"][0]["object"]["root"]["index"],
        0
    );
}

#[test]
fn substitutes_argument_and_capture_effects_and_tracks_returned_aliases() {
    let source = r#"(module
      (type $C (struct (field (mut i32))))
      (func $alias (param $p (ref $C)) (result (ref $C)) (local.get $p))
      (func $set (param $p (ref $C)) (struct.set $C 0 (local.get $p) (i32.const 42)))
      (%closure $Set (%capture $c (ref $C)) (call $set (local.get $c)))
      (func $local (%let $c (struct.new $C (i32.const 0))
        (%closure.call $Set (%closure.new $Set (call $alias (local.get $c)))))))"#;
    let local = memory(source, "$local");
    assert_eq!(local["writes"][0]["object"]["root"]["kind"], "allocation");
    assert_eq!(local["writes"][0]["object"]["root"]["function"], "$local");
    assert!(
        local["allocations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["escapes"]["returned"] == false && a["escapes"]["host"] == false)
    );
    assert_eq!(
        memory(source, "$Set")["writes"][0]["object"]["root"]["kind"],
        "capture"
    );
}

#[test]
fn escape_follows_stores_cycles_and_containers_without_escaping_callee_private_values() {
    let source = r#"(module
      (type $C (struct (field (mut (ref null $C)))))
      (func $store (param $box (ref $C)) (param $value (ref $C))
        (struct.set $C 0 (local.get $box) (local.get $value)))
      (func $private
        (%let $a (struct.new $C (ref.null $C))
          (%let $b (struct.new $C (local.get $a))
            (call $store (local.get $a) (local.get $b)))))
      (func $escape (param $external (ref $C))
        (%let $a (struct.new $C (ref.null $C))
          (%let $b (struct.new $C (local.get $a))
            (call $store (local.get $external) (local.get $b))))))"#;
    let private = memory(source, "$private");
    assert!(
        private["allocations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["escapes"]["stored"] == false)
    );
    let escaping = memory(source, "$escape");
    assert!(
        escaping["allocations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["escapes"]["stored"] == true)
    );
}

#[test]
fn unknown_imports_and_recursive_paths_widen_conservatively() {
    let source = r#"(module
      (type $C (struct (field (mut (ref null $C)))))
      (import "host" "save" (func $save (param $p (ref $C))))
      (func $host (%let $p (struct.new $C (ref.null $C)) (call $save (local.get $p))))
      (func $walk (param $p (ref $C)) (param $n i32)
        (if (i32.eqz (local.get $n))
          (then (struct.set $C 0 (local.get $p) (ref.null $C)))
          (else (call $walk (ref.as_non_null (struct.get $C 0 (local.get $p))) (i32.sub (local.get $n) (i32.const 1)))))))"#;
    let host = memory(source, "$host");
    assert_eq!(host["reads"][0]["object"]["root"]["kind"], "unknown");
    assert_eq!(host["allocations"][0]["escapes"]["host"], true);
    assert!(
        memory(source, "$walk")["writes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["object"]["root"]["kind"] == "unknown")
    );
}

#[test]
fn containers_and_ready_future_payloads_preserve_reference_aliases() {
    let source = r#"(module (type $C (struct (field (mut i32))))
      (func $wrap (param $p (ref $C)) (result (%option (ref $C))) (%option.some (ref $C) (local.get $p)))
      (func $use (param $p (ref $C))
        (%match (%option (ref $C)) (call $wrap (local.get $p))
          (case none () (nop)) (case some ($v) (struct.set $C 0 (local.get $v) (i32.const 42)))))
      (func $ready (%let $p (struct.new $C (i32.const 0))
        (%let $f (%future.ready (ref $C) (local.get $p))
          (struct.set $C 0 (%future.await (ref $C) (%move $f)) (i32.const 42))))))"#;
    assert_eq!(
        memory(source, "$use")["writes"][0]["object"]["root"]["kind"],
        "parameter"
    );
    assert_eq!(
        memory(source, "$ready")["writes"][0]["object"]["root"]["kind"],
        "allocation"
    );
}

#[test]
fn weak_updates_keep_old_aliases_and_recur_propagates_new_objects() {
    let source = r#"(module
      (type $C (struct (field (mut i32))))
      (type $Box (struct (field (mut (ref null $C)))))
      (func $old (param $p (ref $Box)) (param $q (ref $Box)) (param $v (ref $C)) (result (ref null $C))
        (%let $old (struct.get $Box 0 (local.get $p))
          (block (result (ref null $C)) (struct.set $Box 0 (local.get $q) (local.get $v)) (local.get $old))))
      (func $caller (result (ref null $C))
        (%let $a (struct.new $C (i32.const 0))
          (%let $b (struct.new $C (i32.const 1))
            (%let $box (struct.new $Box (local.get $a)) (call $old (local.get $box) (local.get $box) (local.get $b))))))
      (func $loop (param $p (ref $C)) (param $n i32)
        (%loop $go (bind $value (ref $C) (local.get $p)) (bind $i i32 (local.get $n))
          (if (i32.eqz (local.get $i))
            (then (struct.set $C 0 (local.get $value) (i32.const 42)))
            (else (%recur $go (struct.new $C (i32.const 0)) (i32.sub (local.get $i) (i32.const 1))))))))"#;
    let caller = memory(source, "$caller");
    let allocations = caller["allocations"].as_array().unwrap();
    assert!(
        allocations
            .iter()
            .filter(|a| a["ty"] == "(ref $C)")
            .all(|a| a["escapes"]["returned"] == true)
    );
    let looped = memory(source, "$loop");
    let writes = looped["writes"].as_array().unwrap();
    assert!(
        writes
            .iter()
            .any(|w| w["object"]["root"]["kind"] == "parameter")
    );
    assert!(
        writes
            .iter()
            .any(|w| w["object"]["root"]["kind"] == "allocation")
    );
}

#[test]
fn suspension_invalidates_mutable_reference_contents_across_calls() {
    let source = r#"(module
      (type $C (struct (field (mut i32))))
      (type $Box (struct (field (mut (ref null $C)))))
      (import "wlt:host/event-loop@0.1.0" "yield" (func $yield))
      (func $after (param $box (ref $Box))
        (block
          (struct.set $Box 0 (local.get $box) (struct.new $C (i32.const 0)))
          (call $yield)
          (struct.set $C 0 (ref.as_non_null (struct.get $Box 0 (local.get $box))) (i32.const 42)))))"#;
    assert!(
        memory(source, "$after")["writes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["object"]["root"]["kind"] == "unknown")
    );
}
