;; Generated export scaffold. Bodies must be implemented.
(module
  (%use "wlt:core" (type $future))
  (%use "./types.wlt" (type $store.ticket) (type $wit-u32))
  ;; Replace unreachable with the implementation.
  (func $api-run (export "api-run") (result $wit-u32) (%effect pure) (unreachable))
  ;; Replace unreachable with the implementation.
  (func $api-ready (export "api-ready") (result (%inst $future $wit-u32)) (%effect pure) (unreachable))
)
