(module
  (tag $Error (param i64))
  (func $pure-option (param $n i64) (result (option i64)) (effects pure)
    (option.some i64 (local.get $n)))
  (func $raise (effects (throws $Error)) (throw $Error (i64.const 42)))

  (test "option is a pure value"
    (match (option i64) (result i32) (call $pure-option (i64.const 42))
      (case none () (i32.const 0))
      (case some ($value) (i64.eq (local.get $value) (i64.const 42)))))

  (test "result error is ordinary data"
    (match (result unit i32) (result i32) (result.err unit i32 (i32.const 7))
      (case ok () (i32.const 0))
      (case err ($code) (i32.eq (local.get $code) (i32.const 7)))))

  (test "none and some unit remain distinct"
    (match (option unit) (result i32) (option.some unit)
      (case none () (i32.const 0))
      (case some () (i32.const 1))))

  (test "caught throw does not escape"
    (i64.eq
      (try (result i64) (do (block (result i64) (call $raise) (i64.const -1)))
        (catch $Error ($value) (local.get $value)))
      (i64.const 42))))
