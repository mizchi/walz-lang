(module
  (%namespace $json-value
    (%import "builder.wlt"
      (type $builder.Builder)
      (func $builder.new)
      (func $builder.text)
      (func $builder.json)
      (func $builder.finish))
    (%import "bytes.wlt" (func $buffer.equal))
    (%export
      (opaque $Value)
      (type $Kind)
      (func $parse)
      (func $stringify)
      (func $kind)
      (func $null)
      (func $boolean)
      (func $number)
      (func $string)
      (func $array)
      (func $object)
      (func $prepend)
      (func $set)
      (func $get)
      (func $at)
      (func $as-boolean)
      (func $as-number)
      (func $as-string))
    (type $Kind (%variant (null) (boolean) (number) (string) (array) (object)))
    (type $Value
      (%variant
        (null)
        (boolean i32)
        (number %text)
        (string %text)
        (array (ref null $Items))
        (object (ref null $Members))))
    (type $Items (struct (field $value (ref $Value)) (field $next (ref null $Items))))
    (type $Members
      (struct
        (field $key %text)
        (field $value (ref $Value))
        (field $next (ref null $Members))))
    (func $peek (param $b %bytes) (param $p i32) (result i32)
      (%effects reads writes allocates)
      (if
        (result i32)
        (i32.lt_u (local.get $p) (call $bytes-len (local.get $b)))
        (then (%bytes.get (local.get $b) (local.get $p)))
        (else (i32.const -1))))

    (func $bytes-len (param $b %bytes) (result i32) (%effects reads writes allocates)
      (%bytes.len (local.get $b)))

    (func $space (param $b %bytes) (param $start i32) (result i32)
      (%effects reads writes allocates)
      (%loop $ws
        (bind $p i32 (local.get $start))
        (result i32)
        (if
          (result i32)
          (i32.or
            (i32.or
              (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 32))
              (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 9)))
            (i32.or
              (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 10))
              (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 13))))
          (then (%recur $ws (i32.add (local.get $p) (i32.const 1))))
          (else (local.get $p))))
    )

    (func $digits (param $b %bytes) (param $start i32) (result i32)
      (%effects reads writes allocates)
      (%loop $digits
        (bind $p i32 (local.get $start))
        (result i32)
        (if
          (result i32)
          (i32.and
            (i32.ge_u (call $peek (local.get $b) (local.get $p)) (i32.const 48))
            (i32.le_u (call $peek (local.get $b) (local.get $p)) (i32.const 57)))
          (then (%recur $digits (i32.add (local.get $p) (i32.const 1))))
          (else (local.get $p))))
    )

    (func $slice-text (param $b %bytes) (param $start i32) (param $end i32) (result %text)
      (%effects reads writes allocates)
      (%match (%option %text) (result %text)
        (%text.from-bytes
          (%bytes.slice
            (local.get $b)
            (local.get $start)
            (i32.sub (local.get $end) (local.get $start))))
        (case some ($s) (local.get $s))
        (case none () (unreachable))))

    (func $hex-digit (param $x i32) (result i32) (%effects reads writes allocates)
      (if
        (result i32)
        (i32.and
          (i32.ge_u (local.get $x) (i32.const 48))
          (i32.le_u (local.get $x) (i32.const 57)))
        (then (i32.sub (local.get $x) (i32.const 48)))
        (else
          (if
            (result i32)
            (i32.and
              (i32.ge_u (local.get $x) (i32.const 65))
              (i32.le_u (local.get $x) (i32.const 70)))
            (then (i32.sub (local.get $x) (i32.const 55)))
            (else
              (if
                (result i32)
                (i32.and
                  (i32.ge_u (local.get $x) (i32.const 97))
                  (i32.le_u (local.get $x) (i32.const 102)))
                (then (i32.sub (local.get $x) (i32.const 87)))
                (else (i32.const -1))))
          ))))

    (func $hex4 (param $b %bytes) (param $start i32) (result (%result i32 i32))
      (%effects reads writes allocates)
      (%loop $hex
        (bind $p i32 (local.get $start))
        (bind $n i32 (i32.const 0))
        (result (%result i32 i32))
        (if
          (result (%result i32 i32))
          (i32.eq (local.get $p) (i32.add (local.get $start) (i32.const 4)))
          (then (%result.ok i32 i32 (local.get $n)))
          (else
            (%let $d (call $hex-digit (call $peek (local.get $b) (local.get $p)))
              (if
                (result (%result i32 i32))
                (i32.lt_s (local.get $d) (i32.const 0))
                (then (%result.err i32 i32 (local.get $p)))
                (else
                  (%recur $hex
                    (i32.add (local.get $p) (i32.const 1))
                    (i32.add (i32.mul (local.get $n) (i32.const 16)) (local.get $d))))
              ))))
      ))

    (func $codepoint (param $n i32) (result %text) (%effects reads writes allocates)
      (%match (%option %text) (result %text)
        (%text.from-bytes
          (if
            (result %bytes)
            (i32.lt_u (local.get $n) (i32.const 128))
            (then (%bytes.new (local.get $n)))
            (else
              (if
                (result %bytes)
                (i32.lt_u (local.get $n) (i32.const 2048))
                (then
                  (%bytes.new
                    (i32.or (i32.const 192) (i32.shr_u (local.get $n) (i32.const 6)))
                    (i32.or
                      (i32.const 128)
                      (i32.and (i32.shr_u (local.get $n) (i32.const 0)) (i32.const 63))))
                )
                (else
                  (if
                    (result %bytes)
                    (i32.lt_u (local.get $n) (i32.const 65536))
                    (then
                      (%bytes.new
                        (i32.or
                          (i32.const 224)
                          (i32.shr_u (local.get $n) (i32.const 12)))
                        (i32.or
                          (i32.const 128)
                          (i32.and
                            (i32.shr_u (local.get $n) (i32.const 6))
                            (i32.const 63)))
                        (i32.or
                          (i32.const 128)
                          (i32.and
                            (i32.shr_u (local.get $n) (i32.const 0))
                            (i32.const 63))))
                    )
                    (else
                      (%bytes.new
                        (i32.or
                          (i32.const 240)
                          (i32.shr_u (local.get $n) (i32.const 18)))
                        (i32.or
                          (i32.const 128)
                          (i32.and
                            (i32.shr_u (local.get $n) (i32.const 12))
                            (i32.const 63)))
                        (i32.or
                          (i32.const 128)
                          (i32.and
                            (i32.shr_u (local.get $n) (i32.const 6))
                            (i32.const 63)))
                        (i32.or
                          (i32.const 128)
                          (i32.and
                            (i32.shr_u (local.get $n) (i32.const 0))
                            (i32.const 63))))
                    ))))
            )))
        (case some ($s) (local.get $s))
        (case none () (unreachable))))

    (func $escape (param $b %bytes) (param $p i32) (result (%result (%pair %text i32) i32))
      (%effects reads writes allocates)
      (if
        (result (%result (%pair %text i32) i32))
        (i32.eq
          (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 1)))
          (i32.const 117))
        (then
          (%result.let ($high)
            (call $hex4 (local.get $b) (i32.add (local.get $p) (i32.const 2)))
            (if
              (result (%result (%pair %text i32) i32))
              (i32.and
                (i32.ge_u (local.get $high) (i32.const 55296))
                (i32.le_u (local.get $high) (i32.const 56319)))
              (then
                (if
                  (result (%result (%pair %text i32) i32))
                  (i32.and
                    (i32.eq
                      (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 6)))
                      (i32.const 92))
                    (i32.eq
                      (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 7)))
                      (i32.const 117)))
                  (then
                    (%result.let ($low)
                      (call $hex4 (local.get $b) (i32.add (local.get $p) (i32.const 8)))
                      (if
                        (result (%result (%pair %text i32) i32))
                        (i32.and
                          (i32.ge_u (local.get $low) (i32.const 56320))
                          (i32.le_u (local.get $low) (i32.const 57343)))
                        (then
                          (%result.ok (%pair %text i32) i32
                            (%pair.new %text i32
                              (call $codepoint
                                (i32.add
                                  (i32.const 65536)
                                  (i32.add
                                    (i32.mul
                                      (i32.sub (local.get $high) (i32.const 55296))
                                      (i32.const 1024))
                                    (i32.sub (local.get $low) (i32.const 56320))))
                              )
                              (i32.add (local.get $p) (i32.const 12))))
                        )
                        (else (%result.err (%pair %text i32) i32 (local.get $p))))
                    ))
                  (else (%result.err (%pair %text i32) i32 (local.get $p))))
              )
              (else
                (if
                  (result (%result (%pair %text i32) i32))
                  (i32.and
                    (i32.ge_u (local.get $high) (i32.const 56320))
                    (i32.le_u (local.get $high) (i32.const 57343)))
                  (then (%result.err (%pair %text i32) i32 (local.get $p)))
                  (else
                    (%result.ok (%pair %text i32) i32
                      (%pair.new %text i32
                        (call $codepoint (local.get $high))
                        (i32.add (local.get $p) (i32.const 6))))
                  ))))
          ))
        (else
          (if
            (result (%result (%pair %text i32) i32))
            (i32.eq
              (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 1)))
              (i32.const 116))
            (then
              (%result.ok (%pair %text i32) i32
                (%pair.new %text i32
                  (call $codepoint (i32.const 9))
                  (i32.add (local.get $p) (i32.const 2))))
            )
            (else
              (if
                (result (%result (%pair %text i32) i32))
                (i32.eq
                  (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 1)))
                  (i32.const 114))
                (then
                  (%result.ok (%pair %text i32) i32
                    (%pair.new %text i32
                      (call $codepoint (i32.const 13))
                      (i32.add (local.get $p) (i32.const 2))))
                )
                (else
                  (if
                    (result (%result (%pair %text i32) i32))
                    (i32.eq
                      (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 1)))
                      (i32.const 110))
                    (then
                      (%result.ok (%pair %text i32) i32
                        (%pair.new %text i32
                          (call $codepoint (i32.const 10))
                          (i32.add (local.get $p) (i32.const 2))))
                    )
                    (else
                      (if
                        (result (%result (%pair %text i32) i32))
                        (i32.eq
                          (call $peek
                            (local.get $b)
                            (i32.add (local.get $p) (i32.const 1)))
                          (i32.const 102))
                        (then
                          (%result.ok (%pair %text i32) i32
                            (%pair.new %text i32
                              (call $codepoint (i32.const 12))
                              (i32.add (local.get $p) (i32.const 2))))
                        )
                        (else
                          (if
                            (result (%result (%pair %text i32) i32))
                            (i32.eq
                              (call $peek
                                (local.get $b)
                                (i32.add (local.get $p) (i32.const 1)))
                              (i32.const 98))
                            (then
                              (%result.ok (%pair %text i32) i32
                                (%pair.new %text i32
                                  (call $codepoint (i32.const 8))
                                  (i32.add (local.get $p) (i32.const 2))))
                            )
                            (else
                              (if
                                (result (%result (%pair %text i32) i32))
                                (i32.eq
                                  (call $peek
                                    (local.get $b)
                                    (i32.add (local.get $p) (i32.const 1)))
                                  (i32.const 47))
                                (then
                                  (%result.ok (%pair %text i32) i32
                                    (%pair.new %text i32
                                      (call $codepoint (i32.const 47))
                                      (i32.add (local.get $p) (i32.const 2))))
                                )
                                (else
                                  (if
                                    (result (%result (%pair %text i32) i32))
                                    (i32.eq
                                      (call $peek
                                        (local.get $b)
                                        (i32.add (local.get $p) (i32.const 1)))
                                      (i32.const 92))
                                    (then
                                      (%result.ok (%pair %text i32) i32
                                        (%pair.new %text i32
                                          (call $codepoint (i32.const 92))
                                          (i32.add (local.get $p) (i32.const 2))))
                                    )
                                    (else
                                      (if
                                        (result (%result (%pair %text i32) i32))
                                        (i32.eq
                                          (call $peek
                                            (local.get $b)
                                            (i32.add (local.get $p) (i32.const 1)))
                                          (i32.const 34))
                                        (then
                                          (%result.ok (%pair %text i32) i32
                                            (%pair.new %text i32
                                              (call $codepoint (i32.const 34))
                                              (i32.add (local.get $p) (i32.const 2))))
                                        )
                                        (else
                                          (%result.err (%pair %text i32) i32
                                            (local.get $p))))
                                    ))))
                            ))))
                    ))))
            ))))
    )

    (func $parse-string (param $b %bytes) (param $quote i32)
      (result (%result (%pair %text i32) i32)) (%effects reads writes allocates)
      (%let $out (call $builder.new)
        (%loop $string
          (bind $p i32 (i32.add (local.get $quote) (i32.const 1)))
          (bind $start i32 (i32.add (local.get $quote) (i32.const 1)))
          (result (%result (%pair %text i32) i32))
          (if
            (result (%result (%pair %text i32) i32))
            (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const -1))
            (then (%result.err (%pair %text i32) i32 (local.get $p)))
            (else
              (if
                (result (%result (%pair %text i32) i32))
                (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 34))
                (then
                  (block
                    (result (%result (%pair %text i32) i32))
                    (call $builder.text
                      (local.get $out)
                      (call $slice-text
                        (local.get $b)
                        (local.get $start)
                        (local.get $p)))
                    (%result.ok (%pair %text i32) i32
                      (%pair.new %text i32
                        (call $builder.finish (local.get $out))
                        (i32.add (local.get $p) (i32.const 1))))
                  ))
                (else
                  (if
                    (result (%result (%pair %text i32) i32))
                    (i32.lt_u (call $peek (local.get $b) (local.get $p)) (i32.const 32))
                    (then (%result.err (%pair %text i32) i32 (local.get $p)))
                    (else
                      (if
                        (result (%result (%pair %text i32) i32))
                        (i32.eq
                          (call $peek (local.get $b) (local.get $p))
                          (i32.const 92))
                        (then
                          (block
                            (result (%result (%pair %text i32) i32))
                            (call $builder.text
                              (local.get $out)
                              (call $slice-text
                                (local.get $b)
                                (local.get $start)
                                (local.get $p)))
                            (%result.let ($e)
                              (call $escape (local.get $b) (local.get $p))
                              (%let ($s $next) (local.get $e)
                                (block
                                  (result (%result (%pair %text i32) i32))
                                  (call $builder.text (local.get $out) (local.get $s))
                                  (%recur $string (local.get $next) (local.get $next))))
                            )))
                        (else
                          (%recur $string
                            (i32.add (local.get $p) (i32.const 1))
                            (local.get $start))))
                    ))))
            ))))
    )

    (func $parse-number (param $b %bytes) (param $start i32)
      (result (%result (%pair (ref $Value) i32) i32)) (%effects reads writes allocates)
      (%let $sign
        (if (result i32) (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 45)) (then (i32.add (local.get $start) (i32.const 1))) (else (local.get $start)))
        (%result.let ($integer)
          (if
            (result (%result i32 i32))
            (i32.eq (call $peek (local.get $b) (local.get $sign)) (i32.const 48))
            (then (%result.ok i32 i32 (i32.add (local.get $sign) (i32.const 1))))
            (else
              (if
                (result (%result i32 i32))
                (i32.and
                  (i32.ge_u
                    (call $peek (local.get $b) (local.get $sign))
                    (i32.const 49))
                  (i32.le_u
                    (call $peek (local.get $b) (local.get $sign))
                    (i32.const 57)))
                (then
                  (%result.ok i32 i32 (call $digits (local.get $b) (local.get $sign))))
                (else (%result.err i32 i32 (local.get $sign))))
            ))
          (%result.let ($fraction)
            (if
              (result (%result i32 i32))
              (i32.eq (call $peek (local.get $b) (local.get $integer)) (i32.const 46))
              (then
                (if
                  (result (%result i32 i32))
                  (i32.and
                    (i32.ge_u
                      (call $peek
                        (local.get $b)
                        (i32.add (local.get $integer) (i32.const 1)))
                      (i32.const 48))
                    (i32.le_u
                      (call $peek
                        (local.get $b)
                        (i32.add (local.get $integer) (i32.const 1)))
                      (i32.const 57)))
                  (then
                    (%result.ok i32 i32
                      (call $digits
                        (local.get $b)
                        (i32.add (local.get $integer) (i32.const 1))))
                  )
                  (else
                    (%result.err i32 i32 (i32.add (local.get $integer) (i32.const 1))))
                ))
              (else (%result.ok i32 i32 (local.get $integer))))
            (%result.let ($end)
              (if
                (result (%result i32 i32))
                (i32.or
                  (i32.eq
                    (call $peek (local.get $b) (local.get $fraction))
                    (i32.const 101))
                  (i32.eq
                    (call $peek (local.get $b) (local.get $fraction))
                    (i32.const 69)))
                (then
                  (%let $sign (i32.add (local.get $fraction) (i32.const 1))
                    (%let $digits
                      (if (result i32) (i32.or (i32.eq (call $peek (local.get $b) (local.get $sign)) (i32.const 43)) (i32.eq (call $peek (local.get $b) (local.get $sign)) (i32.const 45))) (then (i32.add (local.get $sign) (i32.const 1))) (else (local.get $sign)))
                      (if
                        (result (%result i32 i32))
                        (i32.and
                          (i32.ge_u
                            (call $peek (local.get $b) (local.get $digits))
                            (i32.const 48))
                          (i32.le_u
                            (call $peek (local.get $b) (local.get $digits))
                            (i32.const 57)))
                        (then
                          (%result.ok i32 i32
                            (call $digits (local.get $b) (local.get $digits))))
                        (else (%result.err i32 i32 (local.get $digits))))
                    )))
                (else (%result.ok i32 i32 (local.get $fraction))))
              (%result.ok (%pair (ref $Value) i32) i32
                (%pair.new (ref $Value) i32
                  (%variant.new $Value number
                    (call $slice-text
                      (local.get $b)
                      (local.get $start)
                      (local.get $end)))
                  (local.get $end))))
          ))))

    (func $literal (param $b %bytes) (param $p i32) (param $literal %text)
      (result (%result i32 i32)) (%effects reads writes allocates)
      (%let $word (%text.bytes (local.get $literal))
        (%let $n (%bytes.len (local.get $word))
          (if
            (result (%result i32 i32))
            (i32.le_u
              (local.get $n)
              (i32.sub (%bytes.len (local.get $b)) (local.get $p)))
            (then
              (if
                (result (%result i32 i32))
                (call $buffer.equal
                  (%bytes.slice (local.get $b) (local.get $p) (local.get $n))
                  (local.get $word))
                (then (%result.ok i32 i32 (i32.add (local.get $p) (local.get $n))))
                (else (%result.err i32 i32 (local.get $p))))
            )
            (else (%result.err i32 i32 (local.get $p))))
        )))

    (func $reverse-items (param $head (ref null $Items)) (param $tail (ref null $Items))
      (result (ref null $Items)) (%effects reads writes allocates)
      (%loop $reverse
        (bind $input (ref null $Items) (local.get $head))
        (bind $out (ref null $Items) (local.get $tail))
        (result (ref null $Items))
        (if
          (result (ref null $Items))
          (ref.is_null (local.get $input))
          (then (local.get $out))
          (else
            (%let $node (ref.as_non_null (local.get $input))
              (%recur $reverse
                (struct.get $Items $next (local.get $node))
                (struct.new $Items
                  (struct.get $Items $value (local.get $node))
                  (local.get $out))))
          ))))

    (func $reverse-members (param $head (ref null $Members))
      (param $tail (ref null $Members)) (result (ref null $Members))
      (%effects reads writes allocates)
      (%loop $reverse
        (bind $input (ref null $Members) (local.get $head))
        (bind $out (ref null $Members) (local.get $tail))
        (result (ref null $Members))
        (if
          (result (ref null $Members))
          (ref.is_null (local.get $input))
          (then (local.get $out))
          (else
            (%let $node (ref.as_non_null (local.get $input))
              (%recur $reverse
                (struct.get $Members $next (local.get $node))
                (struct.new $Members
                  (struct.get $Members $key (local.get $node))
                  (struct.get $Members $value (local.get $node))
                  (local.get $out))))
          ))))

    (func $parse-array (param $b %bytes) (param $open i32) (param $depth i32)
      (result (%result (%pair (ref $Value) i32) i32)) (%effects reads writes allocates)
      (%let $start (call $space (local.get $b) (i32.add (local.get $open) (i32.const 1)))
        (if
          (result (%result (%pair (ref $Value) i32) i32))
          (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 93))
          (then
            (%result.ok (%pair (ref $Value) i32) i32
              (%pair.new (ref $Value) i32
                (%variant.new $Value array (ref.null $Items))
                (i32.add (local.get $start) (i32.const 1))))
          )
          (else
            (%loop $read-array
              (bind $p i32 (local.get $start))
              (bind $head (ref null $Items) (ref.null $Items))
              (result (%result (%pair (ref $Value) i32) i32))
              (%let $value-start (local.get $p)
                (%result.let ($parsed)
                  (call $parse-value
                    (local.get $b)
                    (local.get $value-start)
                    (i32.add (local.get $depth) (i32.const 1)))
                  (%let ($item $next) (local.get $parsed)
                    (%let $end (call $space (local.get $b) (local.get $next))
                      (if
                        (result (%result (%pair (ref $Value) i32) i32))
                        (i32.eq
                          (call $peek (local.get $b) (local.get $end))
                          (i32.const 93))
                        (then
                          (%result.ok (%pair (ref $Value) i32) i32
                            (%pair.new (ref $Value) i32
                              (%variant.new $Value array
                                (call $reverse-items
                                  (struct.new $Items
                                    (local.get $item)
                                    (local.get $head))
                                  (ref.null $Items)))
                              (i32.add (local.get $end) (i32.const 1))))
                        )
                        (else
                          (if
                            (result (%result (%pair (ref $Value) i32) i32))
                            (i32.eq
                              (call $peek (local.get $b) (local.get $end))
                              (i32.const 44))
                            (then
                              (%recur $read-array
                                (call $space
                                  (local.get $b)
                                  (i32.add (local.get $end) (i32.const 1)))
                                (struct.new $Items (local.get $item) (local.get $head))))
                            (else
                              (%result.err (%pair (ref $Value) i32) i32 (local.get $end))))
                        ))))
                ))))
        )))

    (func $parse-object (param $b %bytes) (param $open i32) (param $depth i32)
      (result (%result (%pair (ref $Value) i32) i32)) (%effects reads writes allocates)
      (%let $start (call $space (local.get $b) (i32.add (local.get $open) (i32.const 1)))
        (if
          (result (%result (%pair (ref $Value) i32) i32))
          (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 125))
          (then
            (%result.ok (%pair (ref $Value) i32) i32
              (%pair.new (ref $Value) i32
                (%variant.new $Value object (ref.null $Members))
                (i32.add (local.get $start) (i32.const 1))))
          )
          (else
            (%loop $read-object
              (bind $p i32 (local.get $start))
              (bind $head (ref null $Members) (ref.null $Members))
              (result (%result (%pair (ref $Value) i32) i32))
              (if
                (result (%result (%pair (ref $Value) i32) i32))
                (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 34))
                (then
                  (%result.let ($parsed-key)
                    (call $parse-string (local.get $b) (local.get $p))
                    (%let ($key $after-key) (local.get $parsed-key)
                      (%let $colon (call $space (local.get $b) (local.get $after-key))
                        (if
                          (result (%result (%pair (ref $Value) i32) i32))
                          (i32.eq
                            (call $peek (local.get $b) (local.get $colon))
                            (i32.const 58))
                          (then
                            (%let $value-start
                              (call $space (local.get $b) (i32.add (local.get $colon) (i32.const 1)))
                              (%result.let ($parsed)
                                (call $parse-value
                                  (local.get $b)
                                  (local.get $value-start)
                                  (i32.add (local.get $depth) (i32.const 1)))
                                (%let ($item $next) (local.get $parsed)
                                  (%let $end
                                    (call $space (local.get $b) (local.get $next))
                                    (if
                                      (result (%result (%pair (ref $Value) i32) i32))
                                      (i32.eq
                                        (call $peek (local.get $b) (local.get $end))
                                        (i32.const 125))
                                      (then
                                        (%result.ok (%pair (ref $Value) i32) i32
                                          (%pair.new (ref $Value) i32
                                            (%variant.new $Value object
                                              (call $reverse-members
                                                (struct.new $Members
                                                  (local.get $key)
                                                  (local.get $item)
                                                  (local.get $head))
                                                (ref.null $Members)))
                                            (i32.add (local.get $end) (i32.const 1))))
                                      )
                                      (else
                                        (if
                                          (result (%result (%pair (ref $Value) i32) i32))
                                          (i32.eq
                                            (call $peek (local.get $b) (local.get $end))
                                            (i32.const 44))
                                          (then
                                            (%recur $read-object
                                              (call $space
                                                (local.get $b)
                                                (i32.add (local.get $end) (i32.const 1)))
                                              (struct.new $Members
                                                (local.get $key)
                                                (local.get $item)
                                                (local.get $head))))
                                          (else
                                            (%result.err (%pair (ref $Value) i32) i32
                                              (local.get $end))))
                                      ))))
                              )))
                          (else
                            (%result.err (%pair (ref $Value) i32) i32 (local.get $colon))))
                      ))))
                (else (%result.err (%pair (ref $Value) i32) i32 (local.get $p))))
            ))))
    )

    (func $parse-value (param $b %bytes) (param $p i32) (param $depth i32)
      (result (%result (%pair (ref $Value) i32) i32)) (%effects reads writes allocates)
      (if
        (result (%result (%pair (ref $Value) i32) i32))
        (i32.gt_u (local.get $depth) (i32.const 128))
        (then (%result.err (%pair (ref $Value) i32) i32 (local.get $p)))
        (else
          (if
            (result (%result (%pair (ref $Value) i32) i32))
            (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 102))
            (then
              (%result.let ($next)
                (call $literal (local.get $b) (local.get $p) (%text.lit "false"))
                (%result.ok (%pair (ref $Value) i32) i32
                  (%pair.new (ref $Value) i32
                    (%variant.new $Value boolean (i32.const 0))
                    (local.get $next))))
            )
            (else
              (if
                (result (%result (%pair (ref $Value) i32) i32))
                (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 116))
                (then
                  (%result.let ($next)
                    (call $literal (local.get $b) (local.get $p) (%text.lit "true"))
                    (%result.ok (%pair (ref $Value) i32) i32
                      (%pair.new (ref $Value) i32
                        (%variant.new $Value boolean (i32.const 1))
                        (local.get $next))))
                )
                (else
                  (if
                    (result (%result (%pair (ref $Value) i32) i32))
                    (i32.eq (call $peek (local.get $b) (local.get $p)) (i32.const 110))
                    (then
                      (%result.let ($next)
                        (call $literal (local.get $b) (local.get $p) (%text.lit "null"))
                        (%result.ok (%pair (ref $Value) i32) i32
                          (%pair.new (ref $Value) i32
                            (%variant.new $Value null)
                            (local.get $next))))
                    )
                    (else
                      (if
                        (result (%result (%pair (ref $Value) i32) i32))
                        (i32.eq
                          (call $peek (local.get $b) (local.get $p))
                          (i32.const 123))
                        (then
                          (call $parse-object
                            (local.get $b)
                            (local.get $p)
                            (local.get $depth)))
                        (else
                          (if
                            (result (%result (%pair (ref $Value) i32) i32))
                            (i32.eq
                              (call $peek (local.get $b) (local.get $p))
                              (i32.const 91))
                            (then
                              (call $parse-array
                                (local.get $b)
                                (local.get $p)
                                (local.get $depth)))
                            (else
                              (if
                                (result (%result (%pair (ref $Value) i32) i32))
                                (i32.eq
                                  (call $peek (local.get $b) (local.get $p))
                                  (i32.const 34))
                                (then
                                  (%result.let ($s)
                                    (call $parse-string (local.get $b) (local.get $p))
                                    (%let ($text $next) (local.get $s)
                                      (%result.ok (%pair (ref $Value) i32) i32
                                        (%pair.new (ref $Value) i32
                                          (%variant.new $Value string (local.get $text))
                                          (local.get $next))))
                                  ))
                                (else
                                  (call $parse-number (local.get $b) (local.get $p))))
                            ))))
                    ))))
            ))))
    )

    (func $parse (param $source %text) (result (%result (ref $Value) i32))
      (%effects reads writes allocates)
      (%let $b (%text.bytes (local.get $source))
        (if
          (result (%result (ref $Value) i32))
          (i32.gt_u (%bytes.len (local.get $b)) (i32.const 65536))
          (then (%result.err (ref $Value) i32 (i32.const 65536)))
          (else
            (%result.let ($parsed)
              (call $parse-value
                (local.get $b)
                (call $space (local.get $b) (i32.const 0))
                (i32.const 0))
              (%let ($value $next) (local.get $parsed)
                (if
                  (result (%result (ref $Value) i32))
                  (i32.eq
                    (call $space (local.get $b) (local.get $next))
                    (%bytes.len (local.get $b)))
                  (then (%result.ok (ref $Value) i32 (local.get $value)))
                  (else
                    (%result.err (ref $Value) i32
                      (call $space (local.get $b) (local.get $next))))
                ))))
        )))

    (func $null (result (ref $Value)) (%effects reads writes allocates)
      (%variant.new $Value null))

    (func $boolean (param $value i32) (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value boolean (i32.ne (local.get $value) (i32.const 0))))

    (func $string (param $value %text) (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value string (local.get $value)))

    (func $array (result (ref $Value)) (%effects reads writes allocates)
      (%variant.new $Value array (ref.null $Items)))

    (func $object (result (ref $Value)) (%effects reads writes allocates)
      (%variant.new $Value object (ref.null $Members)))

    (func $number (param $lexeme %text) (result (%result (ref $Value) i32))
      (%effects reads writes allocates)
      (%result.let ($value)
        (call $parse (local.get $lexeme))
        (%match $Value (result (%result (ref $Value) i32))
          (local.get $value)
          (case null () (%result.err (ref $Value) i32 (i32.const 0)))
          (case boolean ($x) (%result.err (ref $Value) i32 (i32.const 0)))
          (case number ($x) (%result.ok (ref $Value) i32 (local.get $value)))
          (case string ($x) (%result.err (ref $Value) i32 (i32.const 0)))
          (case array ($x) (%result.err (ref $Value) i32 (i32.const 0)))
          (case object ($x) (%result.err (ref $Value) i32 (i32.const 0))))
      ))

    (func $kind (param $value (ref $Value)) (result (ref $Kind))
      (%effects reads writes allocates)
      (%match $Value (result (ref $Kind))
        (local.get $value)
        (case null () (%variant.new $Kind null))
        (case boolean ($x) (%variant.new $Kind boolean))
        (case number ($x) (%variant.new $Kind number))
        (case string ($x) (%variant.new $Kind string))
        (case array ($x) (%variant.new $Kind array))
        (case object ($x) (%variant.new $Kind object))))

    (func $as-boolean (param $value (ref $Value)) (result (%option i32))
      (%effects reads writes allocates)
      (%match $Value (result (%option i32))
        (local.get $value)
        (case null () (%option.none i32))
        (case boolean ($x) (%option.some i32 (local.get $x)))
        (case number ($x) (%option.none i32))
        (case string ($x) (%option.none i32))
        (case array ($x) (%option.none i32))
        (case object ($x) (%option.none i32))))

    (func $as-number (param $value (ref $Value)) (result (%option %text))
      (%effects reads writes allocates)
      (%match $Value (result (%option %text))
        (local.get $value)
        (case null () (%option.none %text))
        (case boolean ($x) (%option.none %text))
        (case number ($x) (%option.some %text (local.get $x)))
        (case string ($x) (%option.none %text))
        (case array ($x) (%option.none %text))
        (case object ($x) (%option.none %text))))

    (func $as-string (param $value (ref $Value)) (result (%option %text))
      (%effects reads writes allocates)
      (%match $Value (result (%option %text))
        (local.get $value)
        (case null () (%option.none %text))
        (case boolean ($x) (%option.none %text))
        (case number ($x) (%option.none %text))
        (case string ($x) (%option.some %text (local.get $x)))
        (case array ($x) (%option.none %text))
        (case object ($x) (%option.none %text))))

    (func $prepend (param $array (ref $Value)) (param $item (ref $Value))
      (result (%result (ref $Value) i32)) (%effects reads writes allocates)
      (%match $Value (result (%result (ref $Value) i32))
        (local.get $array)
        (case null () (%result.err (ref $Value) i32 (i32.const 0)))
        (case boolean ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case number ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case string ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case array ($x)
          (%result.ok (ref $Value) i32
            (%variant.new $Value array
              (struct.new $Items (local.get $item) (local.get $x))))
        )
        (case object ($x) (%result.err (ref $Value) i32 (i32.const 0))))
    )

    (func $set (param $object (ref $Value)) (param $key %text) (param $item (ref $Value))
      (result (%result (ref $Value) i32)) (%effects reads writes allocates)
      (%match $Value (result (%result (ref $Value) i32))
        (local.get $object)
        (case null () (%result.err (ref $Value) i32 (i32.const 0)))
        (case boolean ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case number ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case string ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case array ($x) (%result.err (ref $Value) i32 (i32.const 0)))
        (case object ($x)
          (%let $reverse (call $reverse-members (local.get $x) (ref.null $Members))
            (%result.ok (ref $Value) i32
              (%variant.new $Value object
                (call $reverse-members
                  (local.get $reverse)
                  (struct.new $Members
                    (local.get $key)
                    (local.get $item)
                    (ref.null $Members))))
            ))))
    )

    (func $get (param $object (ref $Value)) (param $key %text)
      (result (%option (ref $Value))) (%effects reads writes allocates)
      (%match $Value (result (%option (ref $Value)))
        (local.get $object)
        (case null () (%option.none (ref $Value)))
        (case boolean ($x) (%option.none (ref $Value)))
        (case number ($x) (%option.none (ref $Value)))
        (case string ($x) (%option.none (ref $Value)))
        (case array ($x) (%option.none (ref $Value)))
        (case object ($x)
          (%loop $get
            (bind $head (ref null $Members) (local.get $x))
            (bind $found (%option (ref $Value)) (%option.none (ref $Value)))
            (result (%option (ref $Value)))
            (if
              (result (%option (ref $Value)))
              (ref.is_null (local.get $head))
              (then (local.get $found))
              (else
                (%let $node (ref.as_non_null (local.get $head))
                  (%recur $get
                    (struct.get $Members $next (local.get $node))
                    (if
                      (result (%option (ref $Value)))
                      (call $buffer.equal
                        (%text.bytes (local.get $key))
                        (%text.bytes (struct.get $Members $key (local.get $node))))
                      (then
                        (%option.some
                          (ref $Value)
                          (struct.get $Members $value (local.get $node))))
                      (else (local.get $found))))
                ))))
        )))

    (func $at (param $array (ref $Value)) (param $index i32)
      (result (%option (ref $Value))) (%effects reads writes allocates)
      (%match $Value (result (%option (ref $Value)))
        (local.get $array)
        (case null () (%option.none (ref $Value)))
        (case boolean ($x) (%option.none (ref $Value)))
        (case number ($x) (%option.none (ref $Value)))
        (case string ($x) (%option.none (ref $Value)))
        (case array ($x)
          (%loop $at
            (bind $head (ref null $Items) (local.get $x))
            (bind $index i32 (local.get $index))
            (result (%option (ref $Value)))
            (if
              (result (%option (ref $Value)))
              (ref.is_null (local.get $head))
              (then (%option.none (ref $Value)))
              (else
                (%let $node (ref.as_non_null (local.get $head))
                  (if
                    (result (%option (ref $Value)))
                    (i32.eq (local.get $index) (i32.const 0))
                    (then
                      (%option.some
                        (ref $Value)
                        (struct.get $Items $value (local.get $node))))
                    (else
                      (%recur $at
                        (struct.get $Items $next (local.get $node))
                        (i32.sub (local.get $index) (i32.const 1))))
                  ))))
          ))
        (case object ($x) (%option.none (ref $Value))))
    )

    (func $write (param $value (ref $Value)) (param $out (ref $builder.Builder))
      (%effects reads writes allocates)
      (%match $Value
        (local.get $value)
        (case null () (call $builder.text (local.get $out) (%text.lit "null")))
        (case boolean ($x)
          (call $builder.text
            (local.get $out)
            (if
              (result %text)
              (local.get $x)
              (then (%text.lit "true"))
              (else (%text.lit "false"))))
        )
        (case number ($x) (call $builder.text (local.get $out) (local.get $x)))
        (case string ($x) (call $builder.json (local.get $out) (local.get $x)))
        (case array ($x)
          (block
            (call $builder.text (local.get $out) (%text.lit "["))
            (%loop $write-array
              (bind $head (ref null $Items) (local.get $x))
              (bind $first i32 (i32.const 1))
              (if
                (ref.is_null (local.get $head))
                (then (nop))
                (else
                  (%let $node (ref.as_non_null (local.get $head))
                    (block
                      (if
                        (i32.eq (local.get $first) (i32.const 0))
                        (then (call $builder.text (local.get $out) (%text.lit ",")))
                        (else (nop)))
                      (call $write
                        (struct.get $Items $value (local.get $node))
                        (local.get $out))
                      (%recur $write-array
                        (struct.get $Items $next (local.get $node))
                        (i32.const 0))))
                )))
            (call $builder.text (local.get $out) (%text.lit "]"))))
        (case object ($x)
          (block
            (call $builder.text (local.get $out) (%text.lit "{"))
            (%loop $write-object
              (bind $head (ref null $Members) (local.get $x))
              (bind $first i32 (i32.const 1))
              (if
                (ref.is_null (local.get $head))
                (then (nop))
                (else
                  (%let $node (ref.as_non_null (local.get $head))
                    (block
                      (if
                        (i32.eq (local.get $first) (i32.const 0))
                        (then (call $builder.text (local.get $out) (%text.lit ",")))
                        (else (nop)))
                      (call $builder.json
                        (local.get $out)
                        (struct.get $Members $key (local.get $node)))
                      (call $builder.text (local.get $out) (%text.lit ":"))
                      (call $write
                        (struct.get $Members $value (local.get $node))
                        (local.get $out))
                      (%recur $write-object
                        (struct.get $Members $next (local.get $node))
                        (i32.const 0))))
                )))
            (call $builder.text (local.get $out) (%text.lit "}"))))
      ))

    (func $stringify (param $value (ref $Value)) (result %text)
      (%effects reads writes allocates)
      (%let $out (call $builder.new)
        (block
          (result %text)
          (call $write (local.get $value) (local.get $out))
          (call $builder.finish (local.get $out))))
    )))
