(module
  ;; ASCII code points. Grammar: DIGIT+ "+" DIGIT+, with wrapping i64 values.
  (type $Text (array i32))
  (type $Parsed (struct (field $value i64) (field $next i32) (field $valid i32)))

  (func $number (param $text (ref $Text)) (param $start i32) (result (ref $Parsed))
    (%loop $digits (bind $position i32 (local.get $start)) (bind $value i64 (i64.const 0))
      (result (ref $Parsed))
      (if (result (ref $Parsed)) (i32.lt_u (local.get $position) (array.len (local.get $text)))
        (then
          (%let $digit (i32.sub (array.get $Text (local.get $text) (local.get $position)) (i32.const 48))
            (if (result (ref $Parsed)) (i32.lt_u (local.get $digit) (i32.const 10))
              (then (%recur $digits (i32.add (local.get $position) (i32.const 1))
                (i64.add (i64.mul (local.get $value) (i64.const 10))
                  (i64.extend_i32_u (local.get $digit)))))
              (else (struct.new $Parsed (local.get $value) (local.get $position)
                (i32.gt_u (local.get $position) (local.get $start)))))))
        (else (struct.new $Parsed (local.get $value) (local.get $position)
          (i32.gt_u (local.get $position) (local.get $start)))))))

  (func $parse (param $text (ref $Text)) (result i64)
    (%let $left (call $number (local.get $text) (i32.const 0))
      (%let $plus (struct.get $Parsed $next (local.get $left))
        (if (result i64) (i32.and
          (struct.get $Parsed $valid (local.get $left))
          (i32.lt_u (local.get $plus) (array.len (local.get $text))))
          (then
            (if (result i64) (i32.eq (array.get $Text (local.get $text) (local.get $plus)) (i32.const 43))
              (then
                (%let $right (call $number (local.get $text) (i32.add (local.get $plus) (i32.const 1)))
                  (if (result i64) (i32.and
                    (struct.get $Parsed $valid (local.get $right))
                    (i32.eq (struct.get $Parsed $next (local.get $right)) (array.len (local.get $text))))
                    (then (i64.add (struct.get $Parsed $value (local.get $left))
                      (struct.get $Parsed $value (local.get $right))))
                    (else (i64.const -1)))))
              (else (i64.const -1))))
          (else (i64.const -1))))))

  (func $invalid (export "invalid") (result i64)
    (call $parse (array.new_fixed $Text 4
      (i32.const 49) (i32.const 50) (i32.const 43) (i32.const 120))))
  (func $empty (export "empty") (result i64)
    (call $parse (array.new_fixed $Text 0)))
  (func $incomplete (export "incomplete") (result i64)
    (call $parse (array.new_fixed $Text 2 (i32.const 49) (i32.const 43))))
  (func $trailing (export "trailing") (result i64)
    (call $parse (array.new_fixed $Text 4
      (i32.const 49) (i32.const 43) (i32.const 50) (i32.const 33))))
  (func $zero (export "zero") (result i64)
    (call $parse (array.new_fixed $Text 3 (i32.const 48) (i32.const 43) (i32.const 48))))
  (func $main (export "main") (result i64)
    (call $parse (array.new_fixed $Text 5
      (i32.const 49) (i32.const 50) (i32.const 43) (i32.const 51) (i32.const 48)))))
