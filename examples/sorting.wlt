(module
  (type $Ints (array (mut i32)))

  (func $sort (param $xs (ref $Ints))
    (loop $outer (bind $i i32 (i32.const 1))
      (if (i32.lt_u (local.get $i) (array.len (local.get $xs)))
        (then
          (let $key (array.get $Ints (local.get $xs) (local.get $i))
            (block
              (loop $shift (bind $j i32 (local.get $i))
                (if (i32.gt_u (local.get $j) (i32.const 0))
                  (then
                    (let $previous (array.get $Ints (local.get $xs)
                      (i32.sub (local.get $j) (i32.const 1)))
                      (if (i32.gt_s (local.get $previous) (local.get $key))
                        (then (block
                          (array.set $Ints (local.get $xs) (local.get $j) (local.get $previous))
                          (recur $shift (i32.sub (local.get $j) (i32.const 1)))))
                        (else (array.set $Ints (local.get $xs) (local.get $j) (local.get $key))))))
                  (else (array.set $Ints (local.get $xs) (local.get $j) (local.get $key)))))
              (recur $outer (i32.add (local.get $i) (i32.const 1))))))
        (else (nop)))))

  ;; Encode digits after sorting; offset permits testing a negative element.
  (func $sort-and-encode (param $xs (ref $Ints)) (param $offset i32) (result i32)
    (block (result i32)
      (call $sort (local.get $xs))
      (loop $encode (bind $i i32 (i32.const 0)) (bind $value i32 (i32.const 0))
        (result i32)
        (if (result i32) (i32.lt_u (local.get $i) (array.len (local.get $xs)))
          (then (recur $encode
            (i32.add (local.get $i) (i32.const 1))
            (i32.add (i32.mul (local.get $value) (i32.const 10))
              (i32.add (array.get $Ints (local.get $xs) (local.get $i)) (local.get $offset)))))
          (else (local.get $value))))))

  (func $edge (export "edge") (result i32)
    (call $sort-and-encode
      (array.new_fixed $Ints 5 (i32.const -1) (i32.const 3) (i32.const 0) (i32.const 3) (i32.const 1))
      (i32.const 1)))
  (func $empty (export "empty") (result i32)
    (call $sort-and-encode (array.new $Ints (i32.const 0) (i32.const 0)) (i32.const 0)))
  (func $main (export "main") (result i32)
    (call $sort-and-encode
      (array.new_fixed $Ints 5 (i32.const 5) (i32.const 1) (i32.const 4) (i32.const 2) (i32.const 3))
      (i32.const 0))))
