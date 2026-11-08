;; Generated from WIT. Trust comes from bindings.json.
(module
  (%use "wlt:core" (type $future))
  (%use "./types.wlt" (type $store.ticket) (type $wit-u32))
  (%export (type $store.ticket) (type $wit-u32))
  (import "usability:ticket/store" "open" (func $store.open (result (%resource $store.ticket))))
  (%export (func $store.open))
  (import "usability:ticket/store" "size" (func $store.size (param $value (%borrow $store.ticket)) (result $wit-u32)))
  (%export (func $store.size))
)
