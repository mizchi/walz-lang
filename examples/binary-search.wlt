(module
  (type $Ints (array i32))

  (func $search (param $xs (ref $Ints)) (param $key i32) (result i32)
    (%loop $go (bind $low i32 (i32.const 0)) (bind $high i32 (array.len (local.get $xs)))
      (result i32)
      (if (result i32) (i32.ge_u (local.get $low) (local.get $high))
        (then (i32.const -1))
        (else
          (%let $mid (i32.add (local.get $low)
            (i32.div_u (i32.sub (local.get $high) (local.get $low)) (i32.const 2)))
            (%let $value (array.get $Ints (local.get $xs) (local.get $mid))
              (if (result i32) (i32.eq (local.get $value) (local.get $key))
                (then (local.get $mid))
                (else
                  (if (result i32) (i32.lt_s (local.get $value) (local.get $key))
                    (then (%recur $go (i32.add (local.get $mid) (i32.const 1)) (local.get $high)))
                    (else (%recur $go (local.get $low) (local.get $mid))))))))))))

  (func $lookup (export "lookup") (param $key i32) (result i32)
    (call $search
      (array.new_fixed $Ints 5 (i32.const 1) (i32.const 3) (i32.const 5) (i32.const 7) (i32.const 9))
      (local.get $key)))
  (func $main (export "main") (result i32)
    (call $lookup (i32.const 7))))
