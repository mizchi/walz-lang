(module
  (%import "wlt:std"
    (type $list))
  (type $Numbers (array (mut i64)))
  (func $sum (export "sum") (param $values (%inst $list i64)) (result i64) (%effect pure)
    (%loop $sum
      (bind $i i32 (i32.const 0))
      (bind $total i64 (i64.const 0))
      (result i64)
      (if
        (result i64)
        (i32.eq (local.get $i) (%list.len i64 (local.get $values)))
        (then (local.get $total))
        (else
          (%recur $sum
            (i32.add (local.get $i) (i32.const 1))
            (i64.add
              (local.get $total)
              (%list.get i64 (local.get $values) (local.get $i))))
        ))))

  (func $double (export "double") (param $values (%inst $list i64)) (result (%inst $list i64))
    (%effect reads writes allocates)
    (%let $length (%list.len i64 (local.get $values))
      (%let $out (array.new $Numbers (i64.const 0) (local.get $length))
        (block
          (result (%inst $list i64))
          (%loop $map
            (bind $i i32 (i32.const 0))
            (if
              (i32.eq (local.get $i) (local.get $length))
              (then (nop))
              (else
                (block
                  (array.set $Numbers
                    (local.get $out)
                    (local.get $i)
                    (i64.mul
                      (i64.const 2)
                      (%list.get i64 (local.get $values) (local.get $i))))
                  (%recur $map (i32.add (local.get $i) (i32.const 1))))
              )))
          (%list.from-array i64 $Numbers (local.get $out))))
    ))

  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (call $sum (call $double (%list.new i64 (i64.const 20) (i64.const 1))))
  )
  (%test "an empty list maps to an empty list"
    (i32.eqz (%list.len i64 (call $double (%list.new i64))))
  )
  (%test "map and sum preserve signed i64 values" (i64.eq (call $main) (i64.const 42))))
