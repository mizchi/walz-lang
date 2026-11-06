(module
  (%namespace $library
    (%import "wlt:std"
      (type $future))
    (%export (func $answer) (func $unused))
    (import "wlt:host/event-loop@0.1.0" "defer-i64"
      (func $defer (param $delay i32) (param $value i64) (result (%inst $future i64))))
    (func $answer (result i32) (%effects pure) (i32.const 42))

    (func $unused (result (%inst $future i64)) (%effects host)
      (call $defer (i32.const 1) (i64.const 99))))
)
