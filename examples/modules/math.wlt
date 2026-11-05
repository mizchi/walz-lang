(module
  (export (func $answer) (func $unused))
  (func $add (param $a i32) (param $b i32) (result i32) (effects pure)
    (i32.add (local.get $a) (local.get $b)))

  (func $answer (result i32) (effects pure) (call $add (i32.const 40) (i32.const 2)))

  (func $unused (result i32) (effects pure) (i32.const 99)))
