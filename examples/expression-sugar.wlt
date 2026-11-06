(module
  (func $add (param $x i32) (param $y i32) (result i32)
    (%effect pure)
    (i32.add $x $y))

  (func $main (export "main") (result i32)
    (%effect pure)
    (%let (($foo (type i32) 40)
           ($step (type i32) 2))
      (call $add $foo $step)))

  (%test "short expressions" (i32.eq (call $main) 42)))
