(module
  (include "lib.wlt")
  (func $read (param $state (ref $lib.State)) (result i32)
    (call $lib.get (local.get $state))))
