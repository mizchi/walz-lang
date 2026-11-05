(module
  (import "generated/bindings.wlt" (func $service.echo) (func $service.wait))
  (func $echo (export "echo") (param $value (result (pair i64 (option text)) i32))
    (result (result (pair i64 (option text)) i32)) (effects host)
    (call $service.echo (local.get $value)))
  (func $wait (export "wait") (result text) (effects host suspends)
    (call $service.wait)))
