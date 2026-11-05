(module
  (func $lookup (export "lookup") (param $present i32)
    (result (option (result i64 i32))) (effects pure)
    (if (result (option (result i64 i32))) (local.get $present)
      (then (option.some (result i64 i32) (result.ok i64 i32 (i64.const 42))))
      (else (option.none (result i64 i32)))))

  (func $main (export "main") (result i64) (effects pure)
    (match (option (result i64 i32)) (result i64) (call $lookup (i32.const 1))
      (case none () (i64.const -1))
      (case some ($outcome)
        (match (result i64 i32) (result i64) (local.get $outcome)
          (case ok ($value) (local.get $value))
          (case err ($code) (i64.const -2)))))))
