(module
  (func $divide (param $n i64) (param $d i64) (result (result i64 i32)) (effects pure)
    (if (result (result i64 i32)) (i64.eqz (local.get $d))
      (then (result.err i64 i32 (i32.const 1)))
      (else
        (if (result (result i64 i32))
          (i32.and (i64.eq (local.get $n) (i64.const -9223372036854775808))
            (i64.eq (local.get $d) (i64.const -1)))
          (then (result.err i64 i32 (i32.const 2)))
          (else (result.ok i64 i32
            (i64.div_s (local.get $n) (local.get $d))))))))

  (func $error-code (export "error-code") (param $n i64) (param $d i64) (result i32) (effects pure)
    (match (result i64 i32) (result i32) (call $divide (local.get $n) (local.get $d))
      (case ok ($value) (i32.const 0))
      (case err ($code) (local.get $code))))
  (func $main (export "main") (result i64) (effects pure)
    (match (result i64 i32) (result i64) (call $divide (i64.const 84) (i64.const 2))
      (case ok ($value) (local.get $value))
      (case err ($code) (i64.const -1)))))
