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
    ;; Result aliases keep cursor movement separate from AST construction.
    (%type.alias $Parsed (%inst $pair (ref $Value) i32))
    (%type.alias $ParseResult (%inst $result $Parsed i32))
    (%type.alias $OffsetResult (%inst $result i32 i32))
    (%type.alias $StringResult (%inst $result (%inst $pair $text i32) i32))
    (func $peek
      (param $b $bytes)
      (param $p i32)
      (result i32)
      (%effect reads writes allocates)
      (if
        (result i32)
        (i32.lt_u $p (call $bytes-len $b))
        (then (%bytes.get $b $p))
        (else -1)))

    (func $bytes-len
      (param $b $bytes)
      (result i32)
      (%effect reads writes allocates)
      (%bytes.len $b))

    ;; Eager predicate masks keep the tuned scans branchless.
    ;; Unsigned comparisons also preserve the -1 sentinel returned by peek.
    (func $space
      (param $b $bytes)
      (param $start i32)
      (result i32)
      (%effect reads writes allocates)
      (%let $length (%bytes.len $b)
        (%loop $ws
          (bind $p i32 $start)
          (result i32)
          (block
            (result i32)
            (if (i32.ge_u $p $length) (then (return $p)) (else (nop)))
            (%let $c (%bytes.get $b $p)
              (if
                (result i32)
                (| (= $c 32) (| (= $c 9) (| (= $c 10) (= $c 13))))
                (then (%recur $ws (+ $p 1)))
                (else $p))))
        )))

    (func $digits
      (param $b $bytes)
      (param $start i32)
      (result i32)
      (%effect reads writes allocates)
      (%let $length (%bytes.len $b)
        (%loop $digits
          (bind $p i32 $start)
          (result i32)
          (block
            (result i32)
            (if (i32.ge_u $p $length) (then (return $p)) (else (nop)))
            (%let $c (%bytes.get $b $p)
              (if
                (result i32)
                (i32.le_u (- $c 48) 9)
                (then (%recur $digits (+ $p 1)))
                (else $p))))
        )))

    (func $slice-text
      (param $b $bytes)
      (param $start i32)
      (param $end i32)
      (result $text)
      (%effect reads writes allocates)
      (%match (%text.from-bytes (%bytes.slice $b $start (- $end $start)))
        (case some ($s) $s)
        (case none () (unreachable))))

    (func $hex-digit
      (param $x i32)
      (result i32)
      (%effect reads writes allocates)
      (if
        (result i32)
        (& (i32.ge_u $x 48) (i32.le_u $x 57))
        (then (- $x 48))
        (else
          (if
            (result i32)
            (& (i32.ge_u $x 65) (i32.le_u $x 70))
            (then (- $x 55))
            (else
              (if
                (result i32)
                (& (i32.ge_u $x 97) (i32.le_u $x 102))
                (then (- $x 87))
                (else -1))))
        )))

    (func $hex4
      (param $b $bytes)
      (param $start i32)
      (result (%inst $result i32 i32))
      (%effect reads writes allocates)
      (%loop $hex
        (bind $p i32 $start)
        (bind $n i32 0)
        (result (%inst $result i32 i32))
        (if
          (result (%inst $result i32 i32))
          (= $p (+ $start 4))
          (then (%result.ok $n))
          (else
            (%let $d (call $hex-digit (call $peek $b $p))
              (if
                (result (%inst $result i32 i32))
                (< $d 0)
                (then (%result.err $p))
                (else (%recur $hex (+ $p 1) (+ (* $n 16) $d))))
            ))))
    )

    (func $codepoint
      (param $n i32)
      (result $text)
      (%effect reads writes allocates)
      (%match
        (%text.from-bytes
          (if
            (result $bytes)
            (i32.lt_u $n 128)
            (then (%bytes.new $n))
            (else
              (if
                (result $bytes)
                (i32.lt_u $n 2048)
                (then (%bytes.new (| 192 (>>> $n 6)) (| 128 (& (>>> $n 0) 63))))
                (else
                  (if
                    (result $bytes)
                    (i32.lt_u $n 65536)
                    (then
                      (%bytes.new
                        (| 224 (>>> $n 12))
                        (| 128 (& (>>> $n 6) 63))
                        (| 128 (& (>>> $n 0) 63))))
                    (else
                      (%bytes.new
                        (| 240 (>>> $n 18))
                        (| 128 (& (>>> $n 12) 63))
                        (| 128 (& (>>> $n 6) 63))
                        (| 128 (& (>>> $n 0) 63))))
                  ))))
          ))
        (case some ($s) $s)
        (case none () (unreachable))))

    (func $escape-code
      (param $c i32)
      (result i32)
      (%effect pure)
      (block
        (result i32)
        (if (= $c 116) (then (return 9)) (else (nop)))
        (if (= $c 114) (then (return 13)) (else (nop)))
        (if (= $c 110) (then (return 10)) (else (nop)))
        (if (= $c 102) (then (return 12)) (else (nop)))
        (if (= $c 98) (then (return 8)) (else (nop)))
        (if (| (= $c 47) (| (= $c 92) (= $c 34))) (then (return $c)) (else (nop)))
        -1))

    (func $escape-unicode
      (param $b $bytes)
      (param $p i32)
      (result $StringResult)
      (%effect reads writes allocates)
      (%result.do
        ($high (call $hex4 $b (+ $p 2)))
        (block
          (result $StringResult)
          (if
            (& (i32.ge_u $high 56320) (i32.le_u $high 57343))
            (then (return (%result.err $p)))
            (else (nop)))
          (if
            (| (i32.lt_u $high 55296) (i32.gt_u $high 56319))
            (then
              (return (%result.ok (%pair.new (call $codepoint $high) (+ $p 6))))
            )
            (else (nop)))
          (if
            (| (!= (call $peek $b (+ $p 6)) 92) (!= (call $peek $b (+ $p 7)) 117))
            (then (return (%result.err $p)))
            (else (nop)))
          (%result.do
            ($low (call $hex4 $b (+ $p 8)))
            (if
              (result $StringResult)
              (& (i32.ge_u $low 56320) (i32.le_u $low 57343))
              (then
                (%result.ok
                  (%pair.new
                    (call $codepoint
                      (+ 65536 (+ (* (- $high 55296) 1024) (- $low 56320))))
                    (+ $p 12))))
              (else (%result.err $p))))
        )))

    (func $escape
      (param $b $bytes)
      (param $p i32)
      (result $StringResult)
      (%effect reads writes allocates)
      (%let $c (call $peek $b (+ $p 1))
        (block
          (result $StringResult)
          (if (= $c 117) (then (return (call $escape-unicode $b $p))) (else (nop)))
          (%let $code (call $escape-code $c)
            (if
              (result $StringResult)
              (< $code 0)
              (then (%result.err $p))
              (else (%result.ok (%pair.new (call $codepoint $code) (+ $p 2))))
            ))))
    )
    ;; Scan up to eight bytes per iteration. Each load is in bounds;
    ;; return at the first quote, escape or control byte.
    (func $string-end
      (param $b $bytes)
      (param $start i32)
      (result i32)
      (%effect reads writes allocates)
      (%let $length (%bytes.len $b)
        (%loop $scan
          (bind $p i32 $start)
          (result i32)
          (block
            (result i32)
            (if (i32.ge_u $p $length) (then (return $p)) (else (nop)))
            (if
              (result i32)
              (i32.ge_u (- $length $p) 8)
              (then
                (block
                  (result i32)
                  (%let $c (%bytes.get $b $p)
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return $p))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 1))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 1)))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 2))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 2)))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 3))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 3)))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 4))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 4)))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 5))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 5)))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 6))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 6)))
                      (else (nop))))
                  (%let $c (%bytes.get $b (+ $p 7))
                    (if
                      (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                      (then (return (+ $p 7)))
                      (else (nop))))
                  (%recur $scan (+ $p 8))))
              (else
                (%let $c (%bytes.get $b $p)
                  (if
                    (result i32)
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then $p)
                    (else (%recur $scan (+ $p 1))))
                ))))
        )))

    (func $parse-string
      (param $b $bytes)
      (param $quote i32)
      (result $StringResult)
      (%effect reads writes allocates)
      ;; Most strings never need an escape builder or its second copy.
      (%loop $string
        (bind $start i32 (+ $quote 1))
        (bind $out (ref null $builder.Builder) (ref.null $builder.Builder))
        (result $StringResult)
        (%let
          (($end (call $string-end $b $start))
           ($c (call $peek $b $end)))
          (block
            (result $StringResult)
            (if
              (| (= $c -1) (i32.lt_u $c 32))
              (then (return (%result.err $end)))
              (else (nop)))
            (if
              (= $c 34)
              (then
                (return
                  (%result.ok
                    (%pair.new
                      (if
                        (result $text)
                        (ref.is_null $out)
                        (then (call $slice-text $b $start $end))
                        (else
                          (%let $builder (ref.as_non_null $out)
                            (block
                              (result $text)
                              (call $builder.text $builder
                                (call $slice-text $b $start $end))
                              (call $builder.finish $builder))))
                      )
                      (+ $end 1))))
              )
              (else (nop)))
            (%result.do
              ($escaped (call $escape $b $end))
              (%let ($text $next) $escaped
                (%let $builder
                  (if (result (ref $builder.Builder)) (ref.is_null $out) (then (call $builder.new)) (else (ref.as_non_null $out)))
                  (block
                    (result $StringResult)
                    (call $builder.text $builder (call $slice-text $b $start $end))
                    (call $builder.text $builder $text)
                    (%recur $string $next $builder))))
            ))))
    )

    (func $required-digits
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effect reads writes allocates)
      (%let $c (call $peek $b $start)
        (if
          (result $OffsetResult)
          (& (i32.ge_u $c 48) (i32.le_u $c 57))
          (then (%result.ok (call $digits $b $start)))
          (else (%result.err $start))))
    )

    (func $integer-end
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effect reads writes allocates)
      (if
        (result $OffsetResult)
        (= (call $peek $b $start) 48)
        (then (%result.ok (+ $start 1)))
        (else (call $required-digits $b $start))))

    (func $fraction-end
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effect reads writes allocates)
      (if
        (result $OffsetResult)
        (= (call $peek $b $start) 46)
        (then (call $required-digits $b (+ $start 1)))
        (else (%result.ok $start))))

    (func $exponent-end
      (param $b $bytes)
      (param $start i32)
      (result $OffsetResult)
      (%effect reads writes allocates)
      (%let $c (call $peek $b $start)
        (block
          (result $OffsetResult)
          (if
            (i32.eqz (| (= $c 101) (= $c 69)))
            (then (return (%result.ok $start)))
            (else (nop)))
          (%let
            (($sign (+ $start 1))
             ($c (call $peek $b $sign))
             ($digits
               (if (result i32) (| (= $c 43) (= $c 45)) (then (+ $sign 1)) (else $sign))))
            (call $required-digits $b $digits))))
    )

    (func $parse-number
      (param $b $bytes)
      (param $start i32)
      (result $ParseResult)
      (%effect reads writes allocates)
      (%let $sign
        (if (result i32) (= (call $peek $b $start) 45) (then (+ $start 1)) (else $start))
        (%result.do
          ($integer (call $integer-end $b $sign))
          ($fraction (call $fraction-end $b $integer))
          ($end (call $exponent-end $b $fraction))
          (%result.ok
            (%pair.new
              (%variant.new $Value number (call $slice-text $b $start $end))
              $end))))
    )

    (func $literal
      (param $b $bytes)
      (param $p i32)
      (param $literal $text)
      (result (%inst $result i32 i32))
      (%effect reads writes allocates)
      (%let
        (($word (%text.bytes $literal))
         ($n (%bytes.len $word)))
        (if
          (result (%inst $result i32 i32))
          (i32.le_u $n (- (%bytes.len $b) $p))
          (then
            (if
              (result (%inst $result i32 i32))
              (call $buffer.equal (%bytes.slice $b $p $n) $word)
              (then (%result.ok (+ $p $n)))
              (else (%result.err $p))))
          (else (%result.err $p))))
    )

    (func $reverse-items
      (param $head (ref null $Items))
      (param $tail (ref null $Items))
      (result (ref null $Items))
      (%effect reads writes allocates)
      (%loop $reverse
        (bind $input (ref null $Items) $head)
        (bind $out (ref null $Items) $tail)
        (result (ref null $Items))
        (if
          (result (ref null $Items))
          (ref.is_null $input)
          (then $out)
          (else
            (%let $node (ref.as_non_null $input)
              (%recur $reverse
                (struct.get $Items $next $node)
                (struct.new $Items (struct.get $Items $value $node) $out))))
        )))

    (func $reverse-members
      (param $head (ref null $Members))
      (param $tail (ref null $Members))
      (result (ref null $Members))
      (%effect reads writes allocates)
      (%loop $reverse
        (bind $input (ref null $Members) $head)
        (bind $out (ref null $Members) $tail)
        (result (ref null $Members))
        (if
          (result (ref null $Members))
          (ref.is_null $input)
          (then $out)
          (else
            (%let $node (ref.as_non_null $input)
              (%recur $reverse
                (struct.get $Members $next $node)
                (struct.new $Members
                  (struct.get $Members $key $node)
                  (struct.get $Members $value $node)
                  $out))))
        )))

    (func $parse-array
      (param $b $bytes)
      (param $open i32)
      (param $depth i32)
      (result $ParseResult)
      (%effect reads writes allocates)
      (%let $start (call $space $b (+ $open 1))
        (block
          (result $ParseResult)
          (if
            (= (call $peek $b $start) 93)
            (then
              (return
                (%result.ok
                  (%pair.new (%variant.new $Value array (ref.null $Items)) (+ $start 1))))
            )
            (else (nop)))
          ;; Siblings are iterative. Recursion follows nested values only.
          (%loop $read-array
            (bind $p i32 $start)
            (bind $head (ref null $Items) (ref.null $Items))
            (result $ParseResult)
            (%result.do
              ($parsed (call $parse-value $b $p (+ $depth 1)))
              (%let ($item $next) $parsed
                (%let
                  (($head (struct.new $Items $item $head))
                   ($end (call $space $b $next))
                   ($c (call $peek $b $end)))
                  (block
                    (result $ParseResult)
                    (if
                      (= $c 93)
                      (then
                        (return
                          (%result.ok
                            (%pair.new
                              (%variant.new $Value array
                                (call $reverse-items $head (ref.null $Items)))
                              (+ $end 1))))
                      )
                      (else (nop)))
                    (if (!= $c 44) (then (return (%result.err $end))) (else (nop)))
                    (%recur $read-array (call $space $b (+ $end 1)) $head))))
            ))))
    )

    (func $parse-object
      (param $b $bytes)
      (param $open i32)
      (param $depth i32)
      (result $ParseResult)
      (%effect reads writes allocates)
      (%let $start (call $space $b (+ $open 1))
        (block
          (result $ParseResult)
          (if
            (= (call $peek $b $start) 125)
            (then
              (return
                (%result.ok
                  (%pair.new
                    (%variant.new $Value object (ref.null $Members))
                    (+ $start 1))))
            )
            (else (nop)))
          (%loop $read-object
            (bind $p i32 $start)
            (bind $head (ref null $Members) (ref.null $Members))
            (result $ParseResult)
            (block
              (result $ParseResult)
              (if
                (!= (call $peek $b $p) 34)
                (then (return (%result.err $p)))
                (else (nop)))
              (%result.do
                ($parsed-key (call $parse-string $b $p))
                (%let ($key $after-key) $parsed-key
                  (%let $colon (call $space $b $after-key)
                    (block
                      (result $ParseResult)
                      (if
                        (!= (call $peek $b $colon) 58)
                        (then (return (%result.err $colon)))
                        (else (nop)))
                      (%result.do
                        ($parsed
                          (call $parse-value $b
                            (call $space $b (+ $colon 1))
                            (+ $depth 1)))
                        (%let ($item $next) $parsed
                          (%let
                            (($head (struct.new $Members $key $item $head))
                             ($end (call $space $b $next))
                             ($c (call $peek $b $end)))
                            (block
                              (result $ParseResult)
                              (if
                                (= $c 125)
                                (then
                                  (return
                                    (%result.ok
                                      (%pair.new
                                        (%variant.new $Value object
                                          (call $reverse-members $head
                                            (ref.null $Members)))
                                        (+ $end 1))))
                                )
                                (else (nop)))
                              (if
                                (!= $c 44)
                                (then (return (%result.err $end)))
                                (else (nop)))
                              (%recur $read-object (call $space $b (+ $end 1)) $head))))
                      ))))
              ))))
      ))

    (func $parse-value
      (param $b $bytes)
      (param $p i32)
      (param $depth i32)
      (result $ParseResult)
      (%effect reads writes allocates)
      (%let $c (call $peek $b $p)
        (block
          (result $ParseResult)
          ;; Depth counts containers, so an empty container cannot bypass it.
          (if
            (| (i32.gt_u $depth 128) (& (= $depth 128) (| (= $c 123) (= $c 91))))
            (then (return (%result.err $p)))
            (else (nop)))
          (if (= $c 123) (then (return (call $parse-object $b $p $depth))) (else (nop)))
          (if (= $c 91) (then (return (call $parse-array $b $p $depth))) (else (nop)))
          (if
            (= $c 34)
            (then
              (return
                (%result.do
                  ($s (call $parse-string $b $p))
                  (%let ($text $next) $s
                    (%result.ok (%pair.new (%variant.new $Value string $text) $next))))
              ))
            (else (nop)))
          (if
            (= $c 102)
            (then
              (return
                (%result.do
                  ($next (call $literal $b $p (%text.lit "false")))
                  (%result.ok (%pair.new (%variant.new $Value boolean 0) $next))))
            )
            (else (nop)))
          (if
            (= $c 116)
            (then
              (return
                (%result.do
                  ($next (call $literal $b $p (%text.lit "true")))
                  (%result.ok (%pair.new (%variant.new $Value boolean 1) $next))))
            )
            (else (nop)))
          (if
            (= $c 110)
            (then
              (return
                (%result.do
                  ($next (call $literal $b $p (%text.lit "null")))
                  (%result.ok (%pair.new (%variant.new $Value null) $next))))
            )
            (else (nop)))
          (call $parse-number $b $p))))

    (func $parse
      (param $source $text)
      (result (%inst $result (ref $Value) i32))
      (%effect reads writes allocates)
      (%let $b (%text.bytes $source)
        (block
          (result (%inst $result (ref $Value) i32))
          (if
            (i32.gt_u (%bytes.len $b) 65536)
            (then (return (%result.err 65536)))
            (else (nop)))
          (%result.do
            ($parsed (call $parse-value $b (call $space $b 0) 0))
            (%let ($value $next) $parsed
              (%let $end (call $space $b $next)
                (if
                  (result (%inst $result (ref $Value) i32))
                  (= $end (%bytes.len $b))
                  (then (%result.ok $value))
                  (else (%result.err $end))))
            ))))
    )

    (func $null
      (result (ref $Value))
      (%effect reads writes allocates)
      (%variant.new $Value null))

    (func $boolean
      (param $value i32)
      (result (ref $Value))
      (%effect reads writes allocates)
      (%variant.new $Value boolean (!= $value 0)))

    (func $string
      (param $value $text)
      (result (ref $Value))
      (%effect reads writes allocates)
      (%variant.new $Value string $value))

    (func $array
      (result (ref $Value))
      (%effect reads writes allocates)
      (%variant.new $Value array (ref.null $Items)))

    (func $object
      (result (ref $Value))
      (%effect reads writes allocates)
      (%variant.new $Value object (ref.null $Members)))

    (func $number
      (param $lexeme $text)
      (result (%inst $result (ref $Value) i32))
      (%effect reads writes allocates)
      (%result.let ($value)
        (call $parse $lexeme)
        (%match $value
          (case null () (%result.err 0))
          (case boolean ($x) (%result.err 0))
          (case number ($x) (%result.ok $value))
          (case string ($x) (%result.err 0))
          (case array ($x) (%result.err 0))
          (case object ($x) (%result.err 0))))
    )

    (func $kind
      (param $value (ref $Value))
      (result (ref $Kind))
      (%effect reads writes allocates)
      (%match $value
        (case null () (%variant.new $Kind null))
        (case boolean ($x) (%variant.new $Kind boolean))
        (case number ($x) (%variant.new $Kind number))
        (case string ($x) (%variant.new $Kind string))
        (case array ($x) (%variant.new $Kind array))
        (case object ($x) (%variant.new $Kind object))))

    (func $as-boolean
      (param $value (ref $Value))
      (result (%inst $option i32))
      (%effect reads writes allocates)
      (%match $value
        (case null () (%option.none))
        (case boolean ($x) (%option.some $x))
        (case number ($x) (%option.none))
        (case string ($x) (%option.none))
        (case array ($x) (%option.none))
        (case object ($x) (%option.none))))

    (func $as-number
      (param $value (ref $Value))
      (result (%inst $option $text))
      (%effect reads writes allocates)
      (%match $value
        (case null () (%option.none))
        (case boolean ($x) (%option.none))
        (case number ($x) (%option.some $x))
        (case string ($x) (%option.none))
        (case array ($x) (%option.none))
        (case object ($x) (%option.none))))

    (func $as-string
      (param $value (ref $Value))
      (result (%inst $option $text))
      (%effect reads writes allocates)
      (%match $value
        (case null () (%option.none))
        (case boolean ($x) (%option.none))
        (case number ($x) (%option.none))
        (case string ($x) (%option.some $x))
        (case array ($x) (%option.none))
        (case object ($x) (%option.none))))

    (func $prepend
      (param $array (ref $Value))
      (param $item (ref $Value))
      (result (%inst $result (ref $Value) i32))
      (%effect reads writes allocates)
      (%match $array
        (case null () (%result.err 0))
        (case boolean ($x) (%result.err 0))
        (case number ($x) (%result.err 0))
        (case string ($x) (%result.err 0))
        (case array ($x)
          (%result.ok (%variant.new $Value array (struct.new $Items $item $x))))
        (case object ($x) (%result.err 0))))

    (func $set
      (param $object (ref $Value))
      (param $key $text)
      (param $item (ref $Value))
      (result (%inst $result (ref $Value) i32))
      (%effect reads writes allocates)
      (%match $object
        (case null () (%result.err 0))
        (case boolean ($x) (%result.err 0))
        (case number ($x) (%result.err 0))
        (case string ($x) (%result.err 0))
        (case array ($x) (%result.err 0))
        (case object ($x)
          (%let $reverse (call $reverse-members $x (ref.null $Members))
            (%result.ok
              (%variant.new $Value object
                (call $reverse-members $reverse
                  (struct.new $Members $key $item (ref.null $Members))))
            ))))
    )

    (func $get
      (param $object (ref $Value))
      (param $key $text)
      (result (%inst $option (ref $Value)))
      (%effect reads writes allocates)
      (%match $object
        (case null () (%option.none))
        (case boolean ($x) (%option.none))
        (case number ($x) (%option.none))
        (case string ($x) (%option.none))
        (case array ($x) (%option.none))
        (case object ($x)
          (%loop $get
            (bind $head (ref null $Members) $x)
            (bind $found (%inst $option (ref $Value)) (%option.none))
            (result (%inst $option (ref $Value)))
            (if
              (result (%inst $option (ref $Value)))
              (ref.is_null $head)
              (then $found)
              (else
                (%let $node (ref.as_non_null $head)
                  (%recur $get
                    (struct.get $Members $next $node)
                    (if
                      (result (%inst $option (ref $Value)))
                      (call $buffer.equal
                        (%text.bytes $key)
                        (%text.bytes (struct.get $Members $key $node)))
                      (then (%option.some (struct.get $Members $value $node)))
                      (else $found))))
              ))))
      ))

    (func $at
      (param $array (ref $Value))
      (param $index i32)
      (result (%inst $option (ref $Value)))
      (%effect reads writes allocates)
      (%match $array
        (case null () (%option.none))
        (case boolean ($x) (%option.none))
        (case number ($x) (%option.none))
        (case string ($x) (%option.none))
        (case array ($x)
          (%loop $at
            (bind $head (ref null $Items) $x)
            (bind $index i32 $index)
            (result (%inst $option (ref $Value)))
            (if
              (result (%inst $option (ref $Value)))
              (ref.is_null $head)
              (then (%option.none))
              (else
                (%let $node (ref.as_non_null $head)
                  (if
                    (result (%inst $option (ref $Value)))
                    (= $index 0)
                    (then (%option.some (struct.get $Items $value $node)))
                    (else (%recur $at (struct.get $Items $next $node) (- $index 1))))
                ))))
        )
        (case object ($x) (%option.none))))

    (func $write
      (param $value (ref $Value))
      (param $out (ref $builder.Builder))
      (%effect reads writes allocates)
      (%match $value
        (case null () (call $builder.text $out (%text.lit "null")))
        (case boolean ($x)
          (call $builder.text $out
            (if (result $text) $x (then (%text.lit "true")) (else (%text.lit "false"))))
        )
        (case number ($x) (call $builder.text $out $x))
        (case string ($x) (call $builder.json $out $x))
        (case array ($x)
          (block
            (call $builder.text $out (%text.lit "["))
            (%loop $write-array
              (bind $head (ref null $Items) $x)
              (bind $first i32 1)
              (if
                (ref.is_null $head)
                (then (nop))
                (else
                  (%let $node (ref.as_non_null $head)
                    (block
                      (if
                        (= $first 0)
                        (then (call $builder.text $out (%text.lit ",")))
                        (else (nop)))
                      (call $write (struct.get $Items $value $node) $out)
                      (%recur $write-array (struct.get $Items $next $node) 0))))
              ))
            (call $builder.text $out (%text.lit "]"))))
        (case object ($x)
          (block
            (call $builder.text $out (%text.lit "{"))
            (%loop $write-object
              (bind $head (ref null $Members) $x)
              (bind $first i32 1)
              (if
                (ref.is_null $head)
                (then (nop))
                (else
                  (%let $node (ref.as_non_null $head)
                    (block
                      (if
                        (= $first 0)
                        (then (call $builder.text $out (%text.lit ",")))
                        (else (nop)))
                      (call $builder.json $out (struct.get $Members $key $node))
                      (call $builder.text $out (%text.lit ":"))
                      (call $write (struct.get $Members $value $node) $out)
                      (%recur $write-object (struct.get $Members $next $node) 0))))
              ))
            (call $builder.text $out (%text.lit "}"))))
      ))

    (func $stringify
      (param $value (ref $Value))
      (result $text)
      (%effect reads writes allocates)
      (%let $out (call $builder.new)
        (block (result $text) (call $write $value $out) (call $builder.finish $out))))
  ))
