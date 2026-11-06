;; Generated from WIT. Trust comes from bindings.json.
(module
  (%import "wlt:std"
    (type $option)
    (type $pair)
    (type $result)
    (type $text))
  (import "example:host/service@1.0.0" "echo" (func $service.echo (param $value (%inst $result (%inst $pair i64 (%inst $option $text)) i32)) (result (%inst $result (%inst $pair i64 (%inst $option $text)) i32))))
  (%export (func $service.echo))
  (import "example:host/service@1.0.0" "wait" (func $service.wait (result $text)))
  (%export (func $service.wait))
)
