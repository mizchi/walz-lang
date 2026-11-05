;; Generated from WIT. Trust comes from bindings.json.
(module
  (import "example:host/service@1.0.0" "echo" (func $service.echo (param $value (%result (%pair i64 (%option %text)) i32)) (result (%result (%pair i64 (%option %text)) i32))))
  (%export (func $service.echo))
  (import "example:host/service@1.0.0" "wait" (func $service.wait (result %text)))
  (%export (func $service.wait))
)
