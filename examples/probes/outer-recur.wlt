(module
  (func $main (export "main") (result i64)
    (%loop $outer (bind $i i32 (i32.const 0)) (result i64)
      (%loop $inner (bind $j i32 (i32.const 0)) (result i64)
        (%recur $outer (i32.add (local.get $i) (i32.const 1)))))))
