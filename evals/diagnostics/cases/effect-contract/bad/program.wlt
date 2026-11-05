(module
  (type $Cell (struct (field $value (mut i32))))
  (func $read (param $cell (ref $Cell)) (result i32)
    (effects pure)
    (struct.get $Cell $value (local.get $cell))))
