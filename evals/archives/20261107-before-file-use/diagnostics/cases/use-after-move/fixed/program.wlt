(module
  (func $identity (param $value i32) (result i32)
    (let $owner (own.new i32 (local.get $value))
      (block (result i32)
        (own.close (move $owner))
        (local.get $value)))))
