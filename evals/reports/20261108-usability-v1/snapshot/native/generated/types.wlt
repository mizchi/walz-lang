;; Generated nominal portable types.
(module
  (%use "wlt:core" (type $future))
  (import "usability:ticket/store" "[resource-drop]ticket" (func $wit-drop-resource0 (param $handle i32)))
  (type $store.ticket (%resource "usability:ticket/store/ticket" $wit-drop-resource0))
  (type $wit-u32 (%newtype i32))
  (%abi.type $wit-u32 (u32))
  (%export (type $store.ticket) (type $wit-u32))
)
