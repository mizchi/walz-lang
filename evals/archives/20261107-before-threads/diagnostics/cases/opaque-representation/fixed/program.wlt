(module
  (%use "./lib.wlt" (type $State $lib.State) (func $new $lib.new) (func $get $lib.get))
  (func $read (param $state (ref $lib.State)) (result i32)
    (call $lib.get (local.get $state))))
