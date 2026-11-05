(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
  (func $sum (param $value i32) (result i32)
    (call $add (local.get $value) (i32.const 2))))
