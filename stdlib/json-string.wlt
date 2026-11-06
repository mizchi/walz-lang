(module
  (%use "wlt:core" (type $string) (type $bytes))
  (%export (func $json-string.quote))
  (type $Array (array (mut i32)))
  (func $json-string.valid (param $bytes $bytes) (result $string) (%effect pure)
    (%match (%string.from-bytes $bytes)
      (case none () (unreachable))
      (case some ($text) $text)))
  (func $json-string.width
    (param $b i32)
    (result i32)
    (%effect pure)
    (if (result i32) (i32.lt_u $b 32)
      (then 6)
      (else (if (result i32) (| (= $b 34) (= $b 92)) (then 2) (else 1)))))

  (func $json-string.hex
    (param $n i32)
    (result i32)
    (%effect pure)
    (if (result i32) (i32.lt_u $n 10) (then (+ $n 48)) (else (+ $n 87))))

  (func $json-string.put
    (param $out (ref $Array))
    (param $i i32)
    (param $b i32)
    (result i32)
    (%effect writes)
    (%let $w (call $json-string.width $b)
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
                    (array.set $Array $out (+ $i 4) (call $json-string.hex (>>> $b 4)))
                    (array.set $Array $out (+ $i 5) (call $json-string.hex (& $b 15))))
                ))))
        )
        (+ $i $w))))

  ;; Prefix bytes before first are already known not to expand. Continue the
  ;; original sizing algorithm there, so late escapes do not rescan the prefix.
  (func $json-string.quote-escaped
    (param $b $bytes)
    (param $first i32)
    (param $length i32)
    (result $string)
    (%effect reads writes allocates)
    (%let
      (($size
        (%loop $size
          (bind $i i32 $first)
          (bind $n i32 (+ $first 2))
          (result i32)
          (if (result i32) (= $i $length)
            (then $n)
            (else
              (%let $w (call $json-string.width (%bytes.get $b $i))
                (if (result i32) (i32.gt_u $n (- -1 $w))
                  (then (unreachable))
                  (else (%recur $size (+ $i 1) (+ $n $w)))))))))
       ($out (array.new $Array 34 $size)))
      (%loop $copy
        (bind $i i32 0)
        (bind $p i32 1)
        (result $string)
        (if (result $string) (= $i $length)
          (then (call $json-string.valid (%bytes.from-array $Array $out)))
          (else (%recur $copy (+ $i 1) (call $json-string.put $out $p (%bytes.get $b $i))))))))

  (func $json-string.quote
    (param $s $string)
    (result $string)
    (%effect reads writes allocates)
    (%let (($b (%string.bytes $s)) ($length (%bytes.len $b)))
      (if (result $string) (i32.gt_u $length -3)
        (then (unreachable))
        (else
          (%let $first
            (%loop $scan
              (bind $i i32 0)
              (result i32)
              (if (result i32) (= $i $length)
                (then $i)
                (else
                  (%let $byte (%bytes.get $b $i)
                    (if (result i32)
                      (| (i32.lt_u $byte 32) (| (= $byte 34) (= $byte 92)))
                      (then $i)
                      (else (%recur $scan (+ $i 1))))))))
            (if (result $string) (!= $first $length)
              (then (call $json-string.quote-escaped $b $first $length))
              ;; No escape expansion: copy directly, without width/put calls.
              (else
                (%let $out (array.new $Array 34 (+ $length 2))
                  (block (result $string)
                    (%for $copy
                      (bind $i i32 0)
                      (while (i32.lt_u $i $length))
                      (step (+ $i 1))
                      (array.set $Array $out (+ $i 1) (%bytes.get $b $i)))
                    (call $json-string.valid (%bytes.from-array $Array $out)))))))))))
)
