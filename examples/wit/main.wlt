(module
  (%import "wlt:std"
    (type $option)
    (type $pair)
    (type $result)
    (type $text))
  (%import "generated/bindings.wlt" (func $service.echo) (func $service.wait))
  (func $echo (export "echo") (param $value (%inst $result (%inst $pair i64 (%inst $option $text)) i32))
    (result (%inst $result (%inst $pair i64 (%inst $option $text)) i32)) (%effects host)
    (call $service.echo (local.get $value)))
  (func $wait (export "wait") (result $text) (%effects host suspends)
    (call $service.wait)))
