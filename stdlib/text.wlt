(module
  (%namespace $string
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
    (func $find (param $s %text) (param $needle %text) (param $start i32)
      (result (%option i32)) (%effects pure)
      (%let $b (%text.bytes (local.get $s))
        (if
          (result (%option i32))
          (i32.gt_u (local.get $start) (%bytes.len (local.get $b)))
          (then (%option.none))
          (else
            (if
              (result (%option i32))
              (if
                (result i32)
                (i32.eq (local.get $start) (%bytes.len (local.get $b)))
                (then (i32.const 0))
                (else
                  (i32.eq
                    (i32.and
                      (%bytes.get (local.get $b) (local.get $start))
                      (i32.const 192))
                    (i32.const 128))))
              (then (%option.none))
              (else
                (call $buffer.find
                  (local.get $b)
                  (%text.bytes (local.get $needle))
                  (local.get $start))))
          ))))

    (func $split-once (param $s %text) (param $needle %text)
      (result (%option (%pair %text %text))) (%effects pure)
      (%match
        (call $buffer.split-once (%text.bytes (local.get $s)) (%text.bytes (local.get $needle)))
        (case none () (%option.none))
        (case some ($parts)
          (%let ($a $b) (local.get $parts)
            (%option.some
              (%pair.new (call $valid (local.get $a)) (call $valid (local.get $b))))
          ))))

    (func $parse-u64 (param $s %text) (result (%option i64)) (%effects pure)
      (call $buffer.parse-u64
        (%text.bytes (local.get $s))
        (i32.const 0)
        (%bytes.len (%text.bytes (local.get $s)))
        (i64.const -1)))

    (func $parse-s64 (param $s %text) (result (%option i64)) (%effects pure)
      (%let $b (%text.bytes (local.get $s))
        (if
          (result (%option i64))
          (i32.eqz (%bytes.len (local.get $b)))
          (then (%option.none))
          (else
            (%let $negative
              (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 45))
              (%match
                (call $buffer.parse-u64 (local.get $b) (local.get $negative) (i32.sub (%bytes.len (local.get $b)) (local.get $negative)) (if (result i64) (local.get $negative) (then (i64.const -9223372036854775808)) (else (i64.const 9223372036854775807))))
                (case none () (%option.none))
                (case some ($n)
                  (%option.some
                    (if
                      (result i64)
                      (local.get $negative)
                      (then (i64.sub (i64.const 0) (local.get $n)))
                      (else (local.get $n))))
                ))))
        )))
    (type $Array (array (mut i32)))
    (func $valid (param $b %bytes) (result %text) (%effects pure)
      (%match (%option %text) (result %text)
        (%text.from-bytes (local.get $b))
        (case none () (unreachable))
        (case some ($s) (local.get $s))))

    (func $concat (param $a %text) (param $b %text) (result %text)
      (%effects reads writes allocates)
      (call $valid
        (call $buffer.concat (%text.bytes (local.get $a)) (%text.bytes (local.get $b))))
    )

    (func $u64 (param $n i64) (result %text) (%effects reads writes allocates)
      (%let $out (array.new $Array (i32.const 48) (i32.const 20))
        (%loop $digits
          (bind $n i64 (local.get $n))
          (bind $i i32 (i32.const 19))
          (result %text)
          (block
            (result %text)
            (array.set $Array
              (local.get $out)
              (local.get $i)
              (i32.add
                (i32.const 48)
                (i32.wrap_i64 (i64.rem_u (local.get $n) (i64.const 10))))
            )
            (if
              (result %text)
              (i64.lt_u (local.get $n) (i64.const 10))
              (then
                (call $valid
                  (%bytes.slice
                    (%bytes.from-array $Array (local.get $out))
                    (local.get $i)
                    (i32.sub (i32.const 20) (local.get $i))))
              )
              (else
                (%recur $digits
                  (i64.div_u (local.get $n) (i64.const 10))
                  (i32.sub (local.get $i) (i32.const 1))))
            ))))
    )

    (func $s64 (param $n i64) (result %text) (%effects reads writes allocates)
      (if
        (result %text)
        (i64.lt_s (local.get $n) (i64.const 0))
        (then
          (call $concat
            (%text.lit "-")
            (call $u64 (i64.sub (i64.const 0) (local.get $n))))
        )
        (else (call $u64 (local.get $n))))
    )

    (func $width (param $b i32) (result i32) (%effects pure)
      (if
        (result i32)
        (i32.lt_u (local.get $b) (i32.const 32))
        (then (i32.const 6))
        (else
          (if
            (result i32)
            (i32.or
              (i32.eq (local.get $b) (i32.const 34))
              (i32.eq (local.get $b) (i32.const 92)))
            (then (i32.const 2))
            (else (i32.const 1))))
      ))

    (func $hex (param $n i32) (result i32) (%effects pure)
      (if
        (result i32)
        (i32.lt_u (local.get $n) (i32.const 10))
        (then (i32.add (local.get $n) (i32.const 48)))
        (else (i32.add (local.get $n) (i32.const 87))))
    )

    (func $put (param $out (ref $Array)) (param $i i32) (param $b i32) (result i32)
      (%effects writes)
      (%let $w (call $width (local.get $b))
        (block
          (result i32)
          (if
            (i32.eq (local.get $w) (i32.const 1))
            (then (array.set $Array (local.get $out) (local.get $i) (local.get $b)))
            (else
              (block
                (array.set $Array (local.get $out) (local.get $i) (i32.const 92))
                (if
                  (i32.eq (local.get $w) (i32.const 2))
                  (then
                    (array.set $Array
                      (local.get $out)
                      (i32.add (local.get $i) (i32.const 1))
                      (local.get $b)))
                  (else
                    (block
                      (array.set $Array
                        (local.get $out)
                        (i32.add (local.get $i) (i32.const 1))
                        (i32.const 117))
                      (array.set $Array
                        (local.get $out)
                        (i32.add (local.get $i) (i32.const 2))
                        (i32.const 48))
                      (array.set $Array
                        (local.get $out)
                        (i32.add (local.get $i) (i32.const 3))
                        (i32.const 48))
                      (array.set $Array
                        (local.get $out)
                        (i32.add (local.get $i) (i32.const 4))
                        (call $hex (i32.shr_u (local.get $b) (i32.const 4))))
                      (array.set $Array
                        (local.get $out)
                        (i32.add (local.get $i) (i32.const 5))
                        (call $hex (i32.and (local.get $b) (i32.const 15))))
                    ))))
            ))
          (i32.add (local.get $i) (local.get $w))))
    )

    (func $json (param $s %text) (result %text) (%effects reads writes allocates)
      (%let $b (%text.bytes (local.get $s))
        (%let $size
          (%loop $size
            (bind $i i32 (i32.const 0))
            (bind $n i32 (i32.const 2))
            (result i32)
            (if
              (result i32)
              (i32.eq (local.get $i) (%bytes.len (local.get $b)))
              (then (local.get $n))
              (else
                (%let $w (call $width (%bytes.get (local.get $b) (local.get $i)))
                  (if
                    (result i32)
                    (i32.gt_u (local.get $n) (i32.sub (i32.const -1) (local.get $w)))
                    (then (unreachable))
                    (else
                      (%recur $size
                        (i32.add (local.get $i) (i32.const 1))
                        (i32.add (local.get $n) (local.get $w))))
                  ))))
          )
          (%let $out (array.new $Array (i32.const 34) (local.get $size))
            (%loop $copy
              (bind $i i32 (i32.const 0))
              (bind $p i32 (i32.const 1))
              (result %text)
              (if
                (result %text)
                (i32.eq (local.get $i) (%bytes.len (local.get $b)))
                (then (call $valid (%bytes.from-array $Array (local.get $out))))
                (else
                  (%recur $copy
                    (i32.add (local.get $i) (i32.const 1))
                    (call $put
                      (local.get $out)
                      (local.get $p)
                      (%bytes.get (local.get $b) (local.get $i))))
                ))))
        ))))
)
