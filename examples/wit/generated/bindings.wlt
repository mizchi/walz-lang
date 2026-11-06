;; Generated from WIT. Trust comes from bindings.json.
(module
  (%use "wlt:core"
    (type $option)
    (type $tuple)
    (type $result)
    (type $string))
  (import "example:host/service@1.0.0" "echo" (func $service.echo (param $value (%inst $result (%inst $tuple i64 (%inst $option $string)) i32)) (result (%inst $result (%inst $tuple i64 (%inst $option $string)) i32))))
  (%export (func $service.echo))
  (import "example:host/service@1.0.0" "wait" (func $service.wait (result $string)))
  (%export (func $service.wait))
)
