(module
  (%namespace $json-value
    (%import "wlt:std"
      (type $bytes)
      (type $option)
      (type $pair)
      (type $result)
      (type $text))
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
        (number $text)
        (string $text)
        (array (ref null $Items))
        (object (ref null $Members))))
    (type $Items (struct (field $value (ref $Value)) (field $next (ref null $Items))))
    (type $Members
      (struct
        (field $key $text)
        (field $value (ref $Value))
        (field $next (ref null $Members))))
    ;; Parser-only chunks amortize temporary storage across 32 entries.
    ;; Published Items/Members keep immutable fields and pure query effects.
    (type $ParseValues (array (mut (ref null $Value))))
    (type $ParseKeys (array (mut $text)))
    (type $ParseChunk
      (struct
        (field $values (ref $ParseValues))
        (field $keys (ref null $ParseKeys))
        (field $previous (ref null $ParseChunk))))
    (type $ParseBuilder
      (struct
        (field $head (mut (ref null $ParseChunk)))
        (field $length (mut i32))
        (field $object i32)))
    ;; Result aliases keep cursor movement separate from AST construction.
    (%type.alias $Parsed (%inst $pair (ref $Value) i32))
    (%type.alias $ParseResult (%inst $result $Parsed i32))
    (%type.alias $OffsetResult (%inst $result i32 i32))
    (%type.alias $StringResult (%inst $result (%inst $pair $text i32) i32))
    (func $peek
      (param $b $bytes)
      (param $p i32)
      (result i32)
      (%effects reads writes allocates)
      (if
        (result i32)
        (i32.lt_u (local.get $p) (call $bytes-len (local.get $b)))
        (then (%bytes.get (local.get $b) (local.get $p)))
        (else (i32.const -1))))

    (func $bytes-len
      (param $b $bytes)
      (result i32)
      (%effects reads writes allocates)
      (%bytes.len (local.get $b)))

    (func $space
      (param $b $bytes)
      (param $start i32)
      (result i32)
      (%effects reads writes allocates)
      (%let $length (%bytes.len (local.get $b))
        (%loop $ws
          (bind $p i32 (local.get $start))
          (result i32)
          (block
            (result i32)
            (if
              (i32.ge_u (local.get $p) (local.get $length))
              (then (return (local.get $p)))
              (else (nop)))
            (%let $c (%bytes.get (local.get $b) (local.get $p))
              (if
                (result i32)
                (i32.or
                  (i32.eq (local.get $c) (i32.const 32))
                  (i32.or
                    (i32.eq (local.get $c) (i32.const 9))
                    (i32.or
                      (i32.eq (local.get $c) (i32.const 10))
                      (i32.eq (local.get $c) (i32.const 13))))
                )
                (then (%recur $ws (i32.add (local.get $p) (i32.const 1))))
                (else (local.get $p))))
          ))))

    (func $digits
      (param $b $bytes)
      (param $start i32)
      (result i32)
      (%effects reads writes allocates)
      (%let $length (%bytes.len (local.get $b))
        (%loop $digits
          (bind $p i32 (local.get $start))
          (result i32)
          (block
            (result i32)
            (if
              (i32.ge_u (local.get $p) (local.get $length))
              (then (return (local.get $p)))
              (else (nop)))
            (%let $c (%bytes.get (local.get $b) (local.get $p))
              (if
                (result i32)
                (i32.le_u (i32.sub (local.get $c) (i32.const 48)) (i32.const 9))
                (then (%recur $digits (i32.add (local.get $p) (i32.const 1))))
                (else (local.get $p))))
          ))))

    (func $slice-text
      (param $b $bytes)
      (param $start i32)
      (param $end i32)
      (result $text)
      (%effects reads writes allocates)
      (%match (%inst $option $text) (result $text)
        (%text.from-bytes
          (%bytes.slice
            (local.get $b)
            (local.get $start)
            (i32.sub (local.get $end) (local.get $start))))
        (case some ($s) (local.get $s))
        (case none () (unreachable))))

    (func $hex-digit
      (param $x i32)
      (result i32)
      (%effects reads writes allocates)
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

    (func $hex4
      (param $b $bytes)
      (param $start i32)
      (result (%inst $result i32 i32))
      (%effects reads writes allocates)
      (%loop $hex
        (bind $p i32 (local.get $start))
        (bind $n i32 (i32.const 0))
        (result (%inst $result i32 i32))
        (if
          (result (%inst $result i32 i32))
          (i32.eq (local.get $p) (i32.add (local.get $start) (i32.const 4)))
          (then (%result.ok i32 i32 (local.get $n)))
          (else
            (%let $d (call $hex-digit (call $peek (local.get $b) (local.get $p)))
              (if
                (result (%inst $result i32 i32))
                (i32.lt_s (local.get $d) (i32.const 0))
                (then (%result.err i32 i32 (local.get $p)))
                (else
                  (%recur $hex
                    (i32.add (local.get $p) (i32.const 1))
                    (i32.add (i32.mul (local.get $n) (i32.const 16)) (local.get $d))))
              ))))
      ))

    (func $codepoint
      (param $n i32)
      (result $text)
      (%effects reads writes allocates)
      (%match (%inst $option $text) (result $text)
        (%text.from-bytes
          (if
            (result $bytes)
            (i32.lt_u (local.get $n) (i32.const 128))
            (then (%bytes.new (local.get $n)))
            (else
              (if
                (result $bytes)
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
                    (result $bytes)
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

    (func $escape-code
      (param $c i32)
      (result i32)
      (%effects pure)
      (block
        (result i32)
        (if
          (i32.eq (local.get $c) (i32.const 116))
          (then (return (i32.const 9)))
          (else (nop)))
        (if
          (i32.eq (local.get $c) (i32.const 114))
          (then (return (i32.const 13)))
          (else (nop)))
        (if
          (i32.eq (local.get $c) (i32.const 110))
          (then (return (i32.const 10)))
          (else (nop)))
        (if
          (i32.eq (local.get $c) (i32.const 102))
          (then (return (i32.const 12)))
          (else (nop)))
        (if
          (i32.eq (local.get $c) (i32.const 98))
          (then (return (i32.const 8)))
          (else (nop)))
        (if
          (i32.or
            (i32.eq (local.get $c) (i32.const 47))
            (i32.or
              (i32.eq (local.get $c) (i32.const 92))
              (i32.eq (local.get $c) (i32.const 34))))
          (then (return (local.get $c)))
          (else (nop)))
        (i32.const -1)))

    (func $escape-unicode
      (param $b $bytes)
      (param $p i32)
      (result $StringResult)
      (%effects reads writes allocates)
      (%result.do
        ($high (call $hex4 (local.get $b) (i32.add (local.get $p) (i32.const 2))))
        (block
          (result $StringResult)
          (if
            (i32.and
              (i32.ge_u (local.get $high) (i32.const 56320))
              (i32.le_u (local.get $high) (i32.const 57343)))
            (then (return (%result.err (local.get $p))))
            (else (nop)))
          (if
            (i32.or
              (i32.lt_u (local.get $high) (i32.const 55296))
              (i32.gt_u (local.get $high) (i32.const 56319)))
            (then
              (return
                (%result.ok
                  (%pair.new
                    (call $codepoint (local.get $high))
                    (i32.add (local.get $p) (i32.const 6))))
              ))
            (else (nop)))
          (if
            (i32.or
              (i32.ne
                (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 6)))
                (i32.const 92))
              (i32.ne
                (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 7)))
                (i32.const 117)))
            (then (return (%result.err (local.get $p))))
            (else (nop)))
          (%result.do
            ($low (call $hex4 (local.get $b) (i32.add (local.get $p) (i32.const 8))))
            (if
              (result $StringResult)
              (i32.and
                (i32.ge_u (local.get $low) (i32.const 56320))
                (i32.le_u (local.get $low) (i32.const 57343)))
              (then
                (%result.ok
                  (%pair.new
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
              (else (%result.err (local.get $p))))
          ))))

    (func $escape
      (param $b $bytes)
      (param $p i32)
      (result $StringResult)
      (%effects reads writes allocates)
      (%let $c (call $peek (local.get $b) (i32.add (local.get $p) (i32.const 1)))
        (block
          (result $StringResult)
          (if
            (i32.eq (local.get $c) (i32.const 117))
            (then (return (call $escape-unicode (local.get $b) (local.get $p))))
            (else (nop)))
          (%let $code (call $escape-code (local.get $c))
            (if
              (result $StringResult)
              (i32.lt_s (local.get $code) (i32.const 0))
              (then (%result.err (local.get $p)))
              (else
                (%result.ok
                  (%pair.new
                    (call $codepoint (local.get $code))
                    (i32.add (local.get $p) (i32.const 2))))
              ))))
      ))
    ;; Scan an unescaped span. Each byte is read once, after checking bounds.
    (func $string-end
      (param $b $bytes)
      (param $start i32)
      (result i32)
      (%effects reads writes allocates)
      (%let $length (%bytes.len (local.get $b))
        (%loop $scan
          (bind $p i32 (local.get $start))
          (result i32)
          (block
            (result i32)
            (if
              (i32.ge_u (local.get $p) (local.get $length))
              (then (return (local.get $p)))
              (else (nop)))
            (%let $c (%bytes.get (local.get $b) (local.get $p))
              (if
                (result i32)
                (i32.or
                  (i32.lt_u (local.get $c) (i32.const 32))
                  (i32.or
                    (i32.eq (local.get $c) (i32.const 34))
                    (i32.eq (local.get $c) (i32.const 92))))
                (then (local.get $p))
                (else (%recur $scan (i32.add (local.get $p) (i32.const 1))))
              ))))
      ))

    (func $parse-string
      (param $b $bytes)
      (param $quote i32)
      (result $StringResult)
      (%effects reads writes allocates)
      ;; Most strings never need an escape builder or its second copy.
      (%loop $string
        (bind $start i32 (i32.add (local.get $quote) (i32.const 1)))
        (bind $out (ref null $builder.Builder) (ref.null $builder.Builder))
        (result $StringResult)
        (%let
          (($end (call $string-end (local.get $b) (local.get $start)))
           ($c (call $peek (local.get $b) (local.get $end))))
          (block
            (result $StringResult)
            (if
              (i32.or
                (i32.eq (local.get $c) (i32.const -1))
                (i32.lt_u (local.get $c) (i32.const 32)))
              (then (return (%result.err (local.get $end))))
              (else (nop)))
            (if
              (i32.eq (local.get $c) (i32.const 34))
              (then
                (return
                  (%result.ok
                    (%pair.new
                      (if
                        (result $text)
                        (ref.is_null (local.get $out))
                        (then
                          (call $slice-text
                            (local.get $b)
                            (local.get $start)
                            (local.get $end)))
                        (else
                          (%let $builder (ref.as_non_null (local.get $out))
                            (block
                              (result $text)
                              (call $builder.text
                                (local.get $builder)
                                (call $slice-text
                                  (local.get $b)
                                  (local.get $start)
                                  (local.get $end)))
                              (call $builder.finish (local.get $builder))))
                        ))
                      (i32.add (local.get $end) (i32.const 1))))
                ))
              (else (nop)))
            (%result.do
              ($escaped (call $escape (local.get $b) (local.get $end)))
              (%let ($text $next) (local.get $escaped)
                (%let $builder
                  (if (result (ref $builder.Builder)) (ref.is_null (local.get $out)) (then (call $builder.new)) (else (ref.as_non_null (local.get $out))))
                  (block
                    (result $StringResult)
                    (call $builder.text
                      (local.get $builder)
                      (call $slice-text
                        (local.get $b)
                        (local.get $start)
                        (local.get $end)))
                    (call $builder.text (local.get $builder) (local.get $text))
                    (%recur $string (local.get $next) (local.get $builder))))
              ))))
      ))

    (func $required-digits
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effects reads writes allocates)
      (%let $c (call $peek (local.get $b) (local.get $start))
        (if
          (result $OffsetResult)
          (i32.and
            (i32.ge_u (local.get $c) (i32.const 48))
            (i32.le_u (local.get $c) (i32.const 57)))
          (then (%result.ok (call $digits (local.get $b) (local.get $start))))
          (else (%result.err (local.get $start))))
      ))

    (func $integer-end
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effects reads writes allocates)
      (if
        (result $OffsetResult)
        (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 48))
        (then (%result.ok (i32.add (local.get $start) (i32.const 1))))
        (else (call $required-digits (local.get $b) (local.get $start))))
    )

    (func $fraction-end
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effects reads writes allocates)
      (if
        (result $OffsetResult)
        (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 46))
        (then
          (call $required-digits
            (local.get $b)
            (i32.add (local.get $start) (i32.const 1))))
        (else (%result.ok (local.get $start))))
    )

    (func $exponent-end
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effects reads writes allocates)
      (%let $c (call $peek (local.get $b) (local.get $start))
        (block
          (result $OffsetResult)
          (if
            (i32.eqz
              (i32.or
                (i32.eq (local.get $c) (i32.const 101))
                (i32.eq (local.get $c) (i32.const 69))))
            (then (return (%result.ok (local.get $start))))
            (else (nop)))
          (%let
            (($sign (i32.add (local.get $start) (i32.const 1)))
             ($c (call $peek (local.get $b) (local.get $sign)))
             ($digits
               (if
                 (result i32)
                 (i32.or
                   (i32.eq (local.get $c) (i32.const 43))
                   (i32.eq (local.get $c) (i32.const 45)))
                 (then (i32.add (local.get $sign) (i32.const 1)))
                 (else (local.get $sign))))
            )
            (call $required-digits (local.get $b) (local.get $digits))))
      ))

    (func $parse-number
      (param $b $bytes)
      (param $start i32)
      (result $ParseResult)
      (%effects reads writes allocates)
      (%let $sign
        (if (result i32) (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 45)) (then (i32.add (local.get $start) (i32.const 1))) (else (local.get $start)))
        (%result.do
          ($integer (call $integer-end (local.get $b) (local.get $sign)))
          ($fraction (call $fraction-end (local.get $b) (local.get $integer)))
          ($end (call $exponent-end (local.get $b) (local.get $fraction)))
          (%result.ok
            (%pair.new
              (%variant.new $Value number
                (call $slice-text (local.get $b) (local.get $start) (local.get $end)))
              (local.get $end))))
      ))

    (func $literal
      (param $b $bytes)
      (param $p i32)
      (param $literal $text)
      (result (%inst $result i32 i32))
      (%effects reads writes allocates)
      (%let $word (%text.bytes (local.get $literal))
        (%let $n (%bytes.len (local.get $word))
          (if
            (result (%inst $result i32 i32))
            (i32.le_u
              (local.get $n)
              (i32.sub (%bytes.len (local.get $b)) (local.get $p)))
            (then
              (if
                (result (%inst $result i32 i32))
                (call $buffer.equal
                  (%bytes.slice (local.get $b) (local.get $p) (local.get $n))
                  (local.get $word))
                (then (%result.ok i32 i32 (i32.add (local.get $p) (local.get $n))))
                (else (%result.err i32 i32 (local.get $p))))
            )
            (else (%result.err i32 i32 (local.get $p))))
        )))

    (func $parse-builder
      (param $object i32)
      (result (ref $ParseBuilder))
      (%effects allocates)
      (struct.new $ParseBuilder
        (ref.null $ParseChunk)
        (i32.const 0)
        (local.get $object)))

    (func $push-parsed
      (param $out (ref $ParseBuilder))
      (param $value (ref $Value))
      (param $key $text)
      (%effects reads writes allocates)
      (%let
        (($length (struct.get $ParseBuilder $length (local.get $out)))
         ($index (i32.and (local.get $length) (i32.const 31)))
         ($chunk
           (if
             (result (ref $ParseChunk))
             (i32.eqz (local.get $index))
             (then
               (%let $chunk
                 (struct.new $ParseChunk (array.new $ParseValues (ref.null $Value) (i32.const 32)) (if (result (ref null $ParseKeys)) (struct.get $ParseBuilder $object (local.get $out)) (then (array.new $ParseKeys (%text.lit "") (i32.const 32))) (else (ref.null $ParseKeys))) (struct.get $ParseBuilder $head (local.get $out)))
                 (block
                   (result (ref $ParseChunk))
                   (struct.set $ParseBuilder $head (local.get $out) (local.get $chunk))
                   (local.get $chunk))))
             (else (ref.as_non_null (struct.get $ParseBuilder $head (local.get $out))))
           )))
        (block
          (array.set $ParseValues
            (struct.get $ParseChunk $values (local.get $chunk))
            (local.get $index)
            (local.get $value))
          (%let $keys (struct.get $ParseChunk $keys (local.get $chunk))
            (if
              (ref.is_null (local.get $keys))
              (then (nop))
              (else
                (array.set $ParseKeys
                  (ref.as_non_null (local.get $keys))
                  (local.get $index)
                  (local.get $key))))
          )
          ;; At most 65536 input bytes: the private count cannot overflow.
          (struct.set $ParseBuilder $length
            (local.get $out)
            (i32.add (local.get $length) (i32.const 1))))
      ))

    (func $finish-items
      (param $builder (ref $ParseBuilder))
      (result (ref null $Items))
      (%effects reads writes allocates)
      (%loop $chunks
        (bind $head
          (ref null $ParseChunk)
          (struct.get $ParseBuilder $head (local.get $builder)))
        (bind $remaining i32 (struct.get $ParseBuilder $length (local.get $builder)))
        (bind $out (ref null $Items) (ref.null $Items))
        (result (ref null $Items))
        (if
          (result (ref null $Items))
          (ref.is_null (local.get $head))
          (then (local.get $out))
          (else
            (%let
              (($chunk (ref.as_non_null (local.get $head)))
               ($count
                 (i32.add
                   (i32.and
                     (i32.sub (local.get $remaining) (i32.const 1))
                     (i32.const 31))
                   (i32.const 1)))
               ($items
                 (%loop $items
                   (bind $i i32 (local.get $count))
                   (bind $items (ref null $Items) (local.get $out))
                   (result (ref null $Items))
                   (if
                     (result (ref null $Items))
                     (i32.eqz (local.get $i))
                     (then (local.get $items))
                     (else
                       (%recur $items
                         (i32.sub (local.get $i) (i32.const 1))
                         (struct.new $Items
                           (ref.as_non_null
                             (array.get $ParseValues
                               (struct.get $ParseChunk $values (local.get $chunk))
                               (i32.sub (local.get $i) (i32.const 1))))
                           (local.get $items))))
                   ))))
              (%recur $chunks
                (struct.get $ParseChunk $previous (local.get $chunk))
                (i32.sub (local.get $remaining) (local.get $count))
                (local.get $items))))
        )))

    (func $finish-members
      (param $builder (ref $ParseBuilder))
      (result (ref null $Members))
      (%effects reads writes allocates)
      (%loop $chunks
        (bind $head
          (ref null $ParseChunk)
          (struct.get $ParseBuilder $head (local.get $builder)))
        (bind $remaining i32 (struct.get $ParseBuilder $length (local.get $builder)))
        (bind $out (ref null $Members) (ref.null $Members))
        (result (ref null $Members))
        (if
          (result (ref null $Members))
          (ref.is_null (local.get $head))
          (then (local.get $out))
          (else
            (%let
              (($chunk (ref.as_non_null (local.get $head)))
               ($count
                 (i32.add
                   (i32.and
                     (i32.sub (local.get $remaining) (i32.const 1))
                     (i32.const 31))
                   (i32.const 1)))
               ($members
                 (%loop $members
                   (bind $i i32 (local.get $count))
                   (bind $members (ref null $Members) (local.get $out))
                   (result (ref null $Members))
                   (if
                     (result (ref null $Members))
                     (i32.eqz (local.get $i))
                     (then (local.get $members))
                     (else
                       (%recur $members
                         (i32.sub (local.get $i) (i32.const 1))
                         (struct.new $Members
                           (array.get $ParseKeys
                             (ref.as_non_null
                               (struct.get $ParseChunk $keys (local.get $chunk)))
                             (i32.sub (local.get $i) (i32.const 1)))
                           (ref.as_non_null
                             (array.get $ParseValues
                               (struct.get $ParseChunk $values (local.get $chunk))
                               (i32.sub (local.get $i) (i32.const 1))))
                           (local.get $members))))
                   ))))
              (%recur $chunks
                (struct.get $ParseChunk $previous (local.get $chunk))
                (i32.sub (local.get $remaining) (local.get $count))
                (local.get $members))))
        )))

    (func $reverse-members
      (param $head (ref null $Members))
      (param $tail (ref null $Members))
      (result (ref null $Members))
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

    (func $parse-array
      (param $b $bytes)
      (param $open i32)
      (param $depth i32)
      (result $ParseResult)
      (%effects reads writes allocates)
      (%let $start
        (call $space (local.get $b) (i32.add (local.get $open) (i32.const 1)))
        (block
          (result $ParseResult)
          (if
            (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 93))
            (then
              (return
                (%result.ok
                  (%pair.new
                    (%variant.new $Value array (ref.null $Items))
                    (i32.add (local.get $start) (i32.const 1))))
              ))
            (else (nop)))
          ;; Allocate a builder only when there is more than one entry.
          (%loop $read-array
            (bind $p i32 (local.get $start))
            (bind $out (ref null $ParseBuilder) (ref.null $ParseBuilder))
            (result $ParseResult)
            (%result.do
              ($parsed
                (call $parse-value
                  (local.get $b)
                  (local.get $p)
                  (i32.add (local.get $depth) (i32.const 1))))
              (%let ($item $next) (local.get $parsed)
                (%let
                  (($end (call $space (local.get $b) (local.get $next)))
                   ($c (call $peek (local.get $b) (local.get $end))))
                  (block
                    (result $ParseResult)
                    (if
                      (i32.eq (local.get $c) (i32.const 93))
                      (then
                        (return
                          (%result.ok
                            (%pair.new
                              (%variant.new $Value array
                                (if
                                  (result (ref null $Items))
                                  (ref.is_null (local.get $out))
                                  (then
                                    (struct.new $Items
                                      (local.get $item)
                                      (ref.null $Items)))
                                  (else
                                    (%let $builder (ref.as_non_null (local.get $out))
                                      (block
                                        (result (ref null $Items))
                                        (call $push-parsed
                                          (local.get $builder)
                                          (local.get $item)
                                          (%text.lit ""))
                                        (call $finish-items (local.get $builder))))
                                  )))
                              (i32.add (local.get $end) (i32.const 1))))
                        ))
                      (else (nop)))
                    (if
                      (i32.ne (local.get $c) (i32.const 44))
                      (then (return (%result.err (local.get $end))))
                      (else (nop)))
                    (%let $builder
                      (if (result (ref $ParseBuilder)) (ref.is_null (local.get $out)) (then (call $parse-builder (i32.const 0))) (else (ref.as_non_null (local.get $out))))
                      (block
                        (result $ParseResult)
                        (call $push-parsed
                          (local.get $builder)
                          (local.get $item)
                          (%text.lit ""))
                        (%recur $read-array
                          (call $space
                            (local.get $b)
                            (i32.add (local.get $end) (i32.const 1)))
                          (local.get $builder))))
                  ))))
          ))))

    (func $parse-object
      (param $b $bytes)
      (param $open i32)
      (param $depth i32)
      (result $ParseResult)
      (%effects reads writes allocates)
      (%let $start
        (call $space (local.get $b) (i32.add (local.get $open) (i32.const 1)))
        (block
          (result $ParseResult)
          (if
            (i32.eq (call $peek (local.get $b) (local.get $start)) (i32.const 125))
            (then
              (return
                (%result.ok
                  (%pair.new
                    (%variant.new $Value object (ref.null $Members))
                    (i32.add (local.get $start) (i32.const 1))))
              ))
            (else (nop)))
          (%loop $read-object
            (bind $p i32 (local.get $start))
            (bind $out (ref null $ParseBuilder) (ref.null $ParseBuilder))
            (result $ParseResult)
            (block
              (result $ParseResult)
              (if
                (i32.ne (call $peek (local.get $b) (local.get $p)) (i32.const 34))
                (then (return (%result.err (local.get $p))))
                (else (nop)))
              (%result.do
                ($parsed-key (call $parse-string (local.get $b) (local.get $p)))
                (%let ($key $after-key) (local.get $parsed-key)
                  (%let $colon (call $space (local.get $b) (local.get $after-key))
                    (block
                      (result $ParseResult)
                      (if
                        (i32.ne
                          (call $peek (local.get $b) (local.get $colon))
                          (i32.const 58))
                        (then (return (%result.err (local.get $colon))))
                        (else (nop)))
                      (%result.do
                        ($parsed
                          (call $parse-value
                            (local.get $b)
                            (call $space
                              (local.get $b)
                              (i32.add (local.get $colon) (i32.const 1)))
                            (i32.add (local.get $depth) (i32.const 1))))
                        (%let ($item $next) (local.get $parsed)
                          (%let
                            (($end (call $space (local.get $b) (local.get $next)))
                             ($c (call $peek (local.get $b) (local.get $end))))
                            (block
                              (result $ParseResult)
                              (if
                                (i32.eq (local.get $c) (i32.const 125))
                                (then
                                  (return
                                    (%result.ok
                                      (%pair.new
                                        (%variant.new $Value object
                                          (if
                                            (result (ref null $Members))
                                            (ref.is_null (local.get $out))
                                            (then
                                              (struct.new $Members
                                                (local.get $key)
                                                (local.get $item)
                                                (ref.null $Members)))
                                            (else
                                              (%let $builder
                                                (ref.as_non_null (local.get $out))
                                                (block
                                                  (result (ref null $Members))
                                                  (call $push-parsed
                                                    (local.get $builder)
                                                    (local.get $item)
                                                    (local.get $key))
                                                  (call $finish-members
                                                    (local.get $builder))))
                                            )))
                                        (i32.add (local.get $end) (i32.const 1))))
                                  ))
                                (else (nop)))
                              (if
                                (i32.ne (local.get $c) (i32.const 44))
                                (then (return (%result.err (local.get $end))))
                                (else (nop)))
                              (%let $builder
                                (if (result (ref $ParseBuilder)) (ref.is_null (local.get $out)) (then (call $parse-builder (i32.const 1))) (else (ref.as_non_null (local.get $out))))
                                (block
                                  (result $ParseResult)
                                  (call $push-parsed
                                    (local.get $builder)
                                    (local.get $item)
                                    (local.get $key))
                                  (%recur $read-object
                                    (call $space
                                      (local.get $b)
                                      (i32.add (local.get $end) (i32.const 1)))
                                    (local.get $builder))))
                            ))))
                    ))))
            ))))
    )

    (func $parse-value
      (param $b $bytes)
      (param $p i32)
      (param $depth i32)
      (result $ParseResult)
      (%effects reads writes allocates)
      (%let $c (call $peek (local.get $b) (local.get $p))
        (block
          (result $ParseResult)
          ;; Depth counts containers, so an empty container cannot bypass it.
          (if
            (i32.or
              (i32.gt_u (local.get $depth) (i32.const 128))
              (i32.and
                (i32.eq (local.get $depth) (i32.const 128))
                (i32.or
                  (i32.eq (local.get $c) (i32.const 123))
                  (i32.eq (local.get $c) (i32.const 91))))
            )
            (then (return (%result.err (local.get $p))))
            (else (nop)))
          (if
            (i32.eq (local.get $c) (i32.const 123))
            (then
              (return
                (call $parse-object (local.get $b) (local.get $p) (local.get $depth))))
            (else (nop)))
          (if
            (i32.eq (local.get $c) (i32.const 91))
            (then
              (return
                (call $parse-array (local.get $b) (local.get $p) (local.get $depth))))
            (else (nop)))
          (if
            (i32.eq (local.get $c) (i32.const 34))
            (then
              (return
                (%result.do
                  ($s (call $parse-string (local.get $b) (local.get $p)))
                  (%let ($text $next) (local.get $s)
                    (%result.ok
                      (%pair.new
                        (%variant.new $Value string (local.get $text))
                        (local.get $next))))
                )))
            (else (nop)))
          (if
            (i32.eq (local.get $c) (i32.const 102))
            (then
              (return
                (%result.do
                  ($next
                    (call $literal (local.get $b) (local.get $p) (%text.lit "false")))
                  (%result.ok
                    (%pair.new
                      (%variant.new $Value boolean (i32.const 0))
                      (local.get $next))))
              ))
            (else (nop)))
          (if
            (i32.eq (local.get $c) (i32.const 116))
            (then
              (return
                (%result.do
                  ($next
                    (call $literal (local.get $b) (local.get $p) (%text.lit "true")))
                  (%result.ok
                    (%pair.new
                      (%variant.new $Value boolean (i32.const 1))
                      (local.get $next))))
              ))
            (else (nop)))
          (if
            (i32.eq (local.get $c) (i32.const 110))
            (then
              (return
                (%result.do
                  ($next
                    (call $literal (local.get $b) (local.get $p) (%text.lit "null")))
                  (%result.ok (%pair.new (%variant.new $Value null) (local.get $next))))
              ))
            (else (nop)))
          (call $parse-number (local.get $b) (local.get $p))))
    )

    (func $parse
      (param $source $text)
      (result (%inst $result (ref $Value) i32))
      (%effects reads writes allocates)
      (%let $b (%text.bytes (local.get $source))
        (block
          (result (%inst $result (ref $Value) i32))
          (if
            (i32.gt_u (%bytes.len (local.get $b)) (i32.const 65536))
            (then (return (%result.err (i32.const 65536))))
            (else (nop)))
          (%result.do
            ($parsed
              (call $parse-value
                (local.get $b)
                (call $space (local.get $b) (i32.const 0))
                (i32.const 0)))
            (%let ($value $next) (local.get $parsed)
              (%let $end (call $space (local.get $b) (local.get $next))
                (if
                  (result (%inst $result (ref $Value) i32))
                  (i32.eq (local.get $end) (%bytes.len (local.get $b)))
                  (then (%result.ok (local.get $value)))
                  (else (%result.err (local.get $end))))
              ))))
      ))

    (func $null
      (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value null))

    (func $boolean
      (param $value i32)
      (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value boolean (i32.ne (local.get $value) (i32.const 0))))

    (func $string
      (param $value $text)
      (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value string (local.get $value)))

    (func $array
      (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value array (ref.null $Items)))

    (func $object
      (result (ref $Value))
      (%effects reads writes allocates)
      (%variant.new $Value object (ref.null $Members)))

    (func $number
      (param $lexeme $text)
      (result (%inst $result (ref $Value) i32))
      (%effects reads writes allocates)
      (%result.let ($value)
        (call $parse (local.get $lexeme))
        (%match $Value (result (%inst $result (ref $Value) i32))
          (local.get $value)
          (case null () (%result.err (ref $Value) i32 (i32.const 0)))
          (case boolean ($x) (%result.err (ref $Value) i32 (i32.const 0)))
          (case number ($x) (%result.ok (ref $Value) i32 (local.get $value)))
          (case string ($x) (%result.err (ref $Value) i32 (i32.const 0)))
          (case array ($x) (%result.err (ref $Value) i32 (i32.const 0)))
          (case object ($x) (%result.err (ref $Value) i32 (i32.const 0))))
      ))

    (func $kind
      (param $value (ref $Value))
      (result (ref $Kind))
      (%effects reads writes allocates)
      (%match $Value (result (ref $Kind))
        (local.get $value)
        (case null () (%variant.new $Kind null))
        (case boolean ($x) (%variant.new $Kind boolean))
        (case number ($x) (%variant.new $Kind number))
        (case string ($x) (%variant.new $Kind string))
        (case array ($x) (%variant.new $Kind array))
        (case object ($x) (%variant.new $Kind object))))

    (func $as-boolean
      (param $value (ref $Value))
      (result (%inst $option i32))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $option i32))
        (local.get $value)
        (case null () (%option.none i32))
        (case boolean ($x) (%option.some i32 (local.get $x)))
        (case number ($x) (%option.none i32))
        (case string ($x) (%option.none i32))
        (case array ($x) (%option.none i32))
        (case object ($x) (%option.none i32))))

    (func $as-number
      (param $value (ref $Value))
      (result (%inst $option $text))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $option $text))
        (local.get $value)
        (case null () (%option.none $text))
        (case boolean ($x) (%option.none $text))
        (case number ($x) (%option.some $text (local.get $x)))
        (case string ($x) (%option.none $text))
        (case array ($x) (%option.none $text))
        (case object ($x) (%option.none $text))))

    (func $as-string
      (param $value (ref $Value))
      (result (%inst $option $text))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $option $text))
        (local.get $value)
        (case null () (%option.none $text))
        (case boolean ($x) (%option.none $text))
        (case number ($x) (%option.none $text))
        (case string ($x) (%option.some $text (local.get $x)))
        (case array ($x) (%option.none $text))
        (case object ($x) (%option.none $text))))

    (func $prepend
      (param $array (ref $Value))
      (param $item (ref $Value))
      (result (%inst $result (ref $Value) i32))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $result (ref $Value) i32))
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

    (func $set
      (param $object (ref $Value))
      (param $key $text)
      (param $item (ref $Value))
      (result (%inst $result (ref $Value) i32))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $result (ref $Value) i32))
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

    (func $get
      (param $object (ref $Value))
      (param $key $text)
      (result (%inst $option (ref $Value)))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $option (ref $Value)))
        (local.get $object)
        (case null () (%option.none (ref $Value)))
        (case boolean ($x) (%option.none (ref $Value)))
        (case number ($x) (%option.none (ref $Value)))
        (case string ($x) (%option.none (ref $Value)))
        (case array ($x) (%option.none (ref $Value)))
        (case object ($x)
          (%loop $get
            (bind $head (ref null $Members) (local.get $x))
            (bind $found (%inst $option (ref $Value)) (%option.none (ref $Value)))
            (result (%inst $option (ref $Value)))
            (if
              (result (%inst $option (ref $Value)))
              (ref.is_null (local.get $head))
              (then (local.get $found))
              (else
                (%let $node (ref.as_non_null (local.get $head))
                  (%recur $get
                    (struct.get $Members $next (local.get $node))
                    (if
                      (result (%inst $option (ref $Value)))
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

    (func $at
      (param $array (ref $Value))
      (param $index i32)
      (result (%inst $option (ref $Value)))
      (%effects reads writes allocates)
      (%match $Value (result (%inst $option (ref $Value)))
        (local.get $array)
        (case null () (%option.none (ref $Value)))
        (case boolean ($x) (%option.none (ref $Value)))
        (case number ($x) (%option.none (ref $Value)))
        (case string ($x) (%option.none (ref $Value)))
        (case array ($x)
          (%loop $at
            (bind $head (ref null $Items) (local.get $x))
            (bind $index i32 (local.get $index))
            (result (%inst $option (ref $Value)))
            (if
              (result (%inst $option (ref $Value)))
              (ref.is_null (local.get $head))
              (then (%option.none (ref $Value)))
              (else
                (%let $node (ref.as_non_null (local.get $head))
                  (if
                    (result (%inst $option (ref $Value)))
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

    (func $write
      (param $value (ref $Value))
      (param $out (ref $builder.Builder))
      (%effects reads writes allocates)
      (%match $Value
        (local.get $value)
        (case null () (call $builder.text (local.get $out) (%text.lit "null")))
        (case boolean ($x)
          (call $builder.text
            (local.get $out)
            (if
              (result $text)
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

    (func $stringify
      (param $value (ref $Value))
      (result $text)
      (%effects reads writes allocates)
      (%let $out (call $builder.new)
        (block
          (result $text)
          (call $write (local.get $value) (local.get $out))
          (call $builder.finish (local.get $out))))
    )))
