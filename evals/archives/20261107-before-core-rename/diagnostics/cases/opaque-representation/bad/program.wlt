(module
  (include "lib.wlt")
  (func $read (param $state (ref $lib.State)) (result i32)
    (struct.get $lib.State $value (local.get $state))))
