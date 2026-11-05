(module
  (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $ms i32)))
  (func $main (export "main") (result i64) (%effects host suspends)
    (%match (%result i64 i32) (result i64) (%result.ok i64 i32 (i64.const 42))
      (case ok ($value) (local.get $value))
      (case err ($code) (block (result i64) (call $sleep (i32.const 1)) (i64.const 0))))))
