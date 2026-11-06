(module
    (%export (opaque $State) (func $new) (func $get))
    (type $State (struct (field $value i32)))
    (func $new (param $value i32) (result (ref $State))
      (struct.new $State (local.get $value)))
    (func $get (param $state (ref $State)) (result i32)
      (struct.get $State $value (local.get $state))))
