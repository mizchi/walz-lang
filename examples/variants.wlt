(module
  (type $Option (%variant (none) (some i64)))

  (func $value (export "value") (param $present i32) (result i64)
    (%match $Option (result i64)
      (if (result (ref $Option)) (local.get $present)
        (then (%variant.new $Option some (i64.const 42)))
        (else (%variant.new $Option none)))
      (case none () (i64.const 0))
      (case some ($value) (local.get $value))))

  (func $main (export "main") (result i64)
    (call $value (i32.const 1))))
