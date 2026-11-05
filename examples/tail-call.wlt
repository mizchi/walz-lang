(module
  (func $down (param $n i64) (result i64)
    (if (result i64) (i64.eqz (local.get $n))
      (then (i64.const 42))
      (else (return_call $down (i64.sub (local.get $n) (i64.const 1))))))
  (func $main (export "main") (result i64)
    (call $down (i64.const 100000))))
