(module
  (%namespace $string
    (%import "wlt:std" (type $bytes) (type $option) (type $pair) (type $text))
    (%import "bytes.wlt"
      (func $buffer.concat)
      (func $buffer.find)
      (func $buffer.parse-u64)
      (func $buffer.split-once))
    (%export
      (func $concat)
      (func $u64)
      (func $s64)
      (func $json)
      (func $find)
      (func $split-once)
      (func $parse-u64)
      (func $parse-s64))
    (func $find
      (param $s $text)
      (param $needle $text)
      (param $start i32)
      (result (%inst $option i32))
      (%effect pure)
      (%let $b (%text.bytes $s)
        (if
          (result (%inst $option i32))
          (i32.gt_u $start (%bytes.len $b))
          (then (%option.none))
          (else
            (if
              (result (%inst $option i32))
              (if
                (result i32)
                (= $start (%bytes.len $b))
                (then 0)
                (else (= (& (%bytes.get $b $start) 192) 128)))
              (then (%option.none))
              (else (call $buffer.find $b (%text.bytes $needle) $start))))
        )))

    (func $split-once
      (param $s $text)
      (param $needle $text)
      (result (%inst $option (%inst $pair $text $text)))
      (%effect pure)
      (%match (call $buffer.split-once (%text.bytes $s) (%text.bytes $needle))
        (case none () (%option.none))
        (case some ($parts)
          (%let ($a $b) $parts
            (%option.some (%pair.new (call $valid $a) (call $valid $b))))
        )))

    (func $parse-u64
      (param $s $text)
      (result (%inst $option i64))
      (%effect pure)
      (call $buffer.parse-u64 (%text.bytes $s) 0 (%bytes.len (%text.bytes $s)) -1))

    (func $parse-s64
      (param $s $text)
      (result (%inst $option i64))
      (%effect pure)
      (%let $b (%text.bytes $s)
        (if
          (result (%inst $option i64))
          (i32.eqz (%bytes.len $b))
          (then (%option.none))
          (else
            (%let $negative (= (%bytes.get $b 0) 45)
              (%match
                (call $buffer.parse-u64 $b $negative
                  (- (%bytes.len $b) $negative)
                  ;; The negative magnitude may reach 2^63; a positive value may not.
                  (if
                    (result i64)
                    $negative
                    (then -9223372036854775808)
                    (else 9223372036854775807)))
                (case none () (%option.none))
                (case some ($n)
                  (%option.some (if (result i64) $negative (then (- 0 $n)) (else $n))))
              ))))
      ))
    (type $Array (array (mut i32)))
    (func $valid
      (param $b $bytes)
      (result $text)
      (%effect pure)
      (%match (%text.from-bytes $b)
        (case none () (unreachable))
        (case some ($s) $s)))

    (func $concat
      (param $a $text)
      (param $b $text)
      (result $text)
      (%effect reads writes allocates)
      (call $valid (call $buffer.concat (%text.bytes $a) (%text.bytes $b))))

    (func $u64
      (param $n i64)
      (result $text)
      (%effect reads writes allocates)
      (%let $out (array.new $Array 48 20)
        (%loop $digits
          (bind $n i64 $n)
          (bind $i i32 19)
          (result $text)
          (block
            (result $text)
            (array.set $Array $out $i (+ 48 (i32.wrap_i64 (i64.rem_u $n 10))))
            (if
              (result $text)
              (i64.lt_u $n 10)
              (then
                (call $valid
                  (%bytes.slice (%bytes.from-array $Array $out) $i (- 20 $i))))
              (else (%recur $digits (i64.div_u $n 10) (- $i 1))))
          ))))

    (func $s64
      (param $n i64)
      (result $text)
      (%effect reads writes allocates)
      (if
        (result $text)
        (< $n 0)
        (then (call $concat (%text.lit "-") (call $u64 (- 0 $n))))
        (else (call $u64 $n))))

    (func $width
      (param $b i32)
      (result i32)
      (%effect pure)
      (if
        (result i32)
        (i32.lt_u $b 32)
        (then 6)
        (else (if (result i32) (| (= $b 34) (= $b 92)) (then 2) (else 1))))
    )

    (func $hex
      (param $n i32)
      (result i32)
      (%effect pure)
      (if (result i32) (i32.lt_u $n 10) (then (+ $n 48)) (else (+ $n 87))))

    (func $put
      (param $out (ref $Array))
      (param $i i32)
      (param $b i32)
      (result i32)
      (%effect writes)
      (%let $w (call $width $b)
        (block
          (result i32)
          (if
            (= $w 1)
            (then (array.set $Array $out $i $b))
            (else
              (block
                (array.set $Array $out $i 92)
                (if
                  (= $w 2)
                  (then (array.set $Array $out (+ $i 1) $b))
                  (else
                    (block
                      (array.set $Array $out (+ $i 1) 117)
                      (array.set $Array $out (+ $i 2) 48)
                      (array.set $Array $out (+ $i 3) 48)
                      (array.set $Array $out (+ $i 4) (call $hex (>>> $b 4)))
                      (array.set $Array $out (+ $i 5) (call $hex (& $b 15))))
                  ))))
          )
          (+ $i $w))))

    (func $json
      (param $s $text)
      (result $text)
      (%effect reads writes allocates)
      (%let
        (($b (%text.bytes $s))
         ($size
           (%loop $size
             (bind $i i32 0)
             (bind $n i32 2)
             (result i32)
             (if
               (result i32)
               (= $i (%bytes.len $b))
               (then $n)
               (else
                 (%let $w (call $width (%bytes.get $b $i))
                   (if
                     (result i32)
                     (i32.gt_u $n (- -1 $w))
                     (then (unreachable))
                     (else (%recur $size (+ $i 1) (+ $n $w))))
                 ))))
         )
         ($out (array.new $Array 34 $size)))
        (%loop $copy
          (bind $i i32 0)
          (bind $p i32 1)
          (result $text)
          (if
            (result $text)
            (= $i (%bytes.len $b))
            (then (call $valid (%bytes.from-array $Array $out)))
            (else (%recur $copy (+ $i 1) (call $put $out $p (%bytes.get $b $i))))
          ))))
  ))
