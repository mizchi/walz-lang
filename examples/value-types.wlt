(module
  (%import "wlt:std"
    (type $option)
    (type $result))
  (func $lookup (export "lookup") (param $present i32)
    (result (%inst $option (%inst $result i64 i32))) (%effect pure)
    (if (result (%inst $option (%inst $result i64 i32))) (local.get $present)
      (then (%option.some (%inst $result i64 i32) (%result.ok i64 i32 (i64.const 42))))
      (else (%option.none (%inst $result i64 i32)))))

  (func $main (export "main") (result i64) (%effect pure)
    (%match (%inst $option (%inst $result i64 i32)) (result i64) (call $lookup (i32.const 1))
      (case none () (i64.const -1))
      (case some ($outcome)
        (%match (%inst $result i64 i32) (result i64) (local.get $outcome)
          (case ok ($value) (local.get $value))
          (case err ($code) (i64.const -2)))))))
