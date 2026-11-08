(module
  (%use "./lib.wlt" (type $State $lib.State) (func $new $lib.new) (func $get $lib.get))
  (func $read (param $state (ref $lib.State)) (result i32)
    (struct.get $lib.State $value (local.get $state))))
