(module
  (%use "./json-string.wlt" (func $json-string.quote))
  (%use "wlt:core"
    (type $bytes)
    (type $option)
    (type $tuple)
    (type $result)
    (type $string))
  (%use "./builder.wlt"
    (type $string-builder)
    (func $string-builder.new)
    (func $string-builder.text)
    (func $string-builder.finish))
  (%use "./bytes.wlt" (func $bytes.equal))
  (%export
    (opaque $json-value)
    (type $json-kind)
    (func $json.parse)
    (func $json.stringify)
    (func $json.kind)
    (func $json.null)
    (func $json.boolean)
    (func $json.number)
    (func $json.from-string)
    (func $json.array)
    (func $json.object)
    (func $json.prepend)
    (func $json.set)
    (func $json.get)
    (func $json.at)
    (func $json.as-boolean)
    (func $json.as-number)
    (func $json.as-string))
  (type $json-kind (%variant (null) (boolean) (number) (string) (array) (object)))
  (type $json-value
    (%variant
      (null)
      (boolean i32)
      (number $string)
      (string $string)
      (array (ref null $Items))
      (object (ref null $Members))))
  (type $Items
    (struct (field $value (ref $json-value)) (field $next (ref null $Items))))
  (type $Members
    (struct
      (field $key $string)
      (field $value (ref $json-value))
      (field $next (ref null $Members))))
  ;; Result aliases keep cursor movement separate from AST construction.
  (%type.alias $Parsed (%inst $tuple (ref $json-value) i32))
  (%type.alias $ParseResult (%inst $result $Parsed i32))
  (%type.alias $OffsetResult (%inst $result i32 i32))
  (%type.alias $StringResult (%inst $result (%inst $tuple $string i32) i32))
  (func $json.peek
    (param $b $bytes)
    (param $p i32)
    (result i32)
    (%effect reads writes allocates)
    (if
      (result i32)
      (i32.lt_u $p (call $json.bytes-len $b))
      (then (%bytes.get $b $p))
      (else -1)))

  (func $json.bytes-len
    (param $b $bytes)
    (result i32)
    (%effect reads writes allocates)
    (%bytes.len $b))
  ;; Eager predicate masks keep the tuned scans branchless.
  ;; Unsigned comparisons also preserve the -1 sentinel returned by peek.
  (func $json.space
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

  (func $json.digits
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

  (func $json.slice-text
    (param $b $bytes)
    (param $start i32)
    (param $end i32)
    (result $string)
    (%effect reads writes allocates)
    (%match (%string.from-bytes (%bytes.slice $b $start (- $end $start)))
      (case some ($s) $s)
      (case none () (unreachable))))

  (func $json.hex-digit
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

  (func $json.hex4
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
          (%let $d (call $json.hex-digit (call $json.peek $b $p))
            (if
              (result (%inst $result i32 i32))
              (< $d 0)
              (then (%result.err $p))
              (else (%recur $hex (+ $p 1) (+ (* $n 16) $d))))
          ))))
  )

  (func $json.codepoint
    (param $n i32)
    (result $string)
    (%effect reads writes allocates)
    (%match
      (%string.from-bytes
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

  (func $json.escape-code
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

  (func $json.escape-unicode
    (param $b $bytes)
    (param $p i32)
    (result $StringResult)
    (%effect reads writes allocates)
    (%result.do
      ($high (call $json.hex4 $b (+ $p 2)))
      (block
        (result $StringResult)
        (if
          (& (i32.ge_u $high 56320) (i32.le_u $high 57343))
          (then (return (%result.err $p)))
          (else (nop)))
        (if
          (| (i32.lt_u $high 55296) (i32.gt_u $high 56319))
          (then
            (return
              (%result.ok (%tuple.new (call $json.codepoint $high) (+ $p 6))))
          )
          (else (nop)))
        (if
          (|
            (!= (call $json.peek $b (+ $p 6)) 92)
            (!= (call $json.peek $b (+ $p 7)) 117))
          (then (return (%result.err $p)))
          (else (nop)))
        (%result.do
          ($low (call $json.hex4 $b (+ $p 8)))
          (if
            (result $StringResult)
            (& (i32.ge_u $low 56320) (i32.le_u $low 57343))
            (then
              (%result.ok
                (%tuple.new
                  (call $json.codepoint
                    (+ 65536 (+ (* (- $high 55296) 1024) (- $low 56320))))
                  (+ $p 12))))
            (else (%result.err $p))))
      )))

  (func $json.escape
    (param $b $bytes)
    (param $p i32)
    (result $StringResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b (+ $p 1))
      (block
        (result $StringResult)
        (if
          (= $c 117)
          (then (return (call $json.escape-unicode $b $p)))
          (else (nop)))
        (%let $code (call $json.escape-code $c)
          (if
            (result $StringResult)
            (< $code 0)
            (then (%result.err $p))
            (else (%result.ok (%tuple.new (call $json.codepoint $code) (+ $p 2))))
          ))))
  )
  ;; Scan up to eight bytes per iteration. Each load is in bounds;
  ;; return at the first quote, escape or control byte.
  (func $json.string-end
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

  (func $json.parse-string
    (param $b $bytes)
    (param $quote i32)
    (result $StringResult)
    (%effect reads writes allocates)
    ;; Most strings never need an escape builder or its second copy.
    (%loop $string
      (bind $start i32 (+ $quote 1))
      (bind $out (ref null $string-builder) (ref.null $string-builder))
      (result $StringResult)
      (%let
        (($end (call $json.string-end $b $start))
         ($c (call $json.peek $b $end)))
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
                  (%tuple.new
                    (if
                      (result $string)
                      (ref.is_null $out)
                      (then (call $json.slice-text $b $start $end))
                      (else
                        (%let $builder (ref.as_non_null $out)
                          (block
                            (result $string)
                            (call $string-builder.text $builder
                              (call $json.slice-text $b $start $end))
                            (call $string-builder.finish $builder))))
                    )
                    (+ $end 1))))
            )
            (else (nop)))
          (%result.do
            ($escaped (call $json.escape $b $end))
            (%let ($text $next) $escaped
              (%let $builder
                (if (result (ref $string-builder)) (ref.is_null $out) (then (call $string-builder.new)) (else (ref.as_non_null $out)))
                (block
                  (result $StringResult)
                  (call $string-builder.text $builder
                    (call $json.slice-text $b $start $end))
                  (call $string-builder.text $builder $text)
                  (%recur $string $next $builder))))
          ))))
  )

  (func $json.required-digits
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b $start)
      (if
        (result $OffsetResult)
        (& (i32.ge_u $c 48) (i32.le_u $c 57))
        (then (%result.ok (call $json.digits $b $start)))
        (else (%result.err $start))))
  )

  (func $json.integer-end
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (if
      (result $OffsetResult)
      (= (call $json.peek $b $start) 48)
      (then (%result.ok (+ $start 1)))
      (else (call $json.required-digits $b $start))))

  (func $json.fraction-end
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (if
      (result $OffsetResult)
      (= (call $json.peek $b $start) 46)
      (then (call $json.required-digits $b (+ $start 1)))
      (else (%result.ok $start))))

  (func $json.exponent-end
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b $start)
      (block
        (result $OffsetResult)
        (if
          (i32.eqz (| (= $c 101) (= $c 69)))
          (then (return (%result.ok $start)))
          (else (nop)))
        (%let
          (($sign (+ $start 1))
           ($c (call $json.peek $b $sign))
           ($digits
             (if (result i32) (| (= $c 43) (= $c 45)) (then (+ $sign 1)) (else $sign))))
          (call $json.required-digits $b $digits))))
  )

  (func $json.parse-number
    (param $b $bytes)
    (param $start i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $sign
      (if (result i32) (= (call $json.peek $b $start) 45) (then (+ $start 1)) (else $start))
      (%result.do
        ($integer (call $json.integer-end $b $sign))
        ($fraction (call $json.fraction-end $b $integer))
        ($end (call $json.exponent-end $b $fraction))
        (%result.ok
          (%tuple.new
            (%variant.new $json-value number
              (call $json.slice-text $b $start $end))
            $end))))
  )

  (func $json.literal
    (param $b $bytes)
    (param $p i32)
    (param $literal $string)
    (result (%inst $result i32 i32))
    (%effect reads writes allocates)
    (%let
      (($word (%string.bytes $literal))
       ($n (%bytes.len $word)))
      (if
        (result (%inst $result i32 i32))
        (i32.le_u $n (- (%bytes.len $b) $p))
        (then
          (if
            (result (%inst $result i32 i32))
            (call $bytes.equal (%bytes.slice $b $p $n) $word)
            (then (%result.ok (+ $p $n)))
            (else (%result.err $p))))
        (else (%result.err $p))))
  )

  (func $json.reverse-items
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

  (func $json.reverse-members
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

  (func $json.parse-array
    (param $b $bytes)
    (param $open i32)
    (param $depth i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $start (call $json.space $b (+ $open 1))
      (block
        (result $ParseResult)
        (if
          (= (call $json.peek $b $start) 93)
          (then
            (return
              (%result.ok
                (%tuple.new
                  (%variant.new $json-value array (ref.null $Items))
                  (+ $start 1))))
          )
          (else (nop)))
        ;; Siblings are iterative. Recursion follows nested values only.
        (%loop $read-array
          (bind $p i32 $start)
          (bind $head (ref null $Items) (ref.null $Items))
          (result $ParseResult)
          (%result.do
            ($parsed (call $json.parse-value $b $p (+ $depth 1)))
            (%let ($item $next) $parsed
              (%let
                (($head (struct.new $Items $item $head))
                 ($end (call $json.space $b $next))
                 ($c (call $json.peek $b $end)))
                (block
                  (result $ParseResult)
                  (if
                    (= $c 93)
                    (then
                      (return
                        (%result.ok
                          (%tuple.new
                            (%variant.new $json-value array
                              (call $json.reverse-items $head (ref.null $Items)))
                            (+ $end 1))))
                    )
                    (else (nop)))
                  (if (!= $c 44) (then (return (%result.err $end))) (else (nop)))
                  (%recur $read-array (call $json.space $b (+ $end 1)) $head))))
          ))))
  )

  (func $json.parse-object
    (param $b $bytes)
    (param $open i32)
    (param $depth i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $start (call $json.space $b (+ $open 1))
      (block
        (result $ParseResult)
        (if
          (= (call $json.peek $b $start) 125)
          (then
            (return
              (%result.ok
                (%tuple.new
                  (%variant.new $json-value object (ref.null $Members))
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
              (!= (call $json.peek $b $p) 34)
              (then (return (%result.err $p)))
              (else (nop)))
            (%result.do
              ($parsed-key (call $json.parse-string $b $p))
              (%let ($key $after-key) $parsed-key
                (%let $colon (call $json.space $b $after-key)
                  (block
                    (result $ParseResult)
                    (if
                      (!= (call $json.peek $b $colon) 58)
                      (then (return (%result.err $colon)))
                      (else (nop)))
                    (%result.do
                      ($parsed
                        (call $json.parse-value $b
                          (call $json.space $b (+ $colon 1))
                          (+ $depth 1)))
                      (%let ($item $next) $parsed
                        (%let
                          (($head (struct.new $Members $key $item $head))
                           ($end (call $json.space $b $next))
                           ($c (call $json.peek $b $end)))
                          (block
                            (result $ParseResult)
                            (if
                              (= $c 125)
                              (then
                                (return
                                  (%result.ok
                                    (%tuple.new
                                      (%variant.new $json-value object
                                        (call $json.reverse-members $head
                                          (ref.null $Members)))
                                      (+ $end 1))))
                              )
                              (else (nop)))
                            (if
                              (!= $c 44)
                              (then (return (%result.err $end)))
                              (else (nop)))
                            (%recur $read-object
                              (call $json.space $b (+ $end 1))
                              $head))))
                    ))))
            ))))
    ))

  (func $json.parse-value
    (param $b $bytes)
    (param $p i32)
    (param $depth i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b $p)
      (block
        (result $ParseResult)
        ;; Depth counts containers, so an empty container cannot bypass it.
        (if
          (| (i32.gt_u $depth 128) (& (= $depth 128) (| (= $c 123) (= $c 91))))
          (then (return (%result.err $p)))
          (else (nop)))
        (if
          (= $c 123)
          (then (return (call $json.parse-object $b $p $depth)))
          (else (nop)))
        (if
          (= $c 91)
          (then (return (call $json.parse-array $b $p $depth)))
          (else (nop)))
        (if
          (= $c 34)
          (then
            (return
              (%result.do
                ($s (call $json.parse-string $b $p))
                (%let ($text $next) $s
                  (%result.ok
                    (%tuple.new (%variant.new $json-value string $text) $next))))
            ))
          (else (nop)))
        (if
          (= $c 102)
          (then
            (return
              (%result.do
                ($next (call $json.literal $b $p (%string.lit "false")))
                (%result.ok (%tuple.new (%variant.new $json-value boolean 0) $next))))
          )
          (else (nop)))
        (if
          (= $c 116)
          (then
            (return
              (%result.do
                ($next (call $json.literal $b $p (%string.lit "true")))
                (%result.ok (%tuple.new (%variant.new $json-value boolean 1) $next))))
          )
          (else (nop)))
        (if
          (= $c 110)
          (then
            (return
              (%result.do
                ($next (call $json.literal $b $p (%string.lit "null")))
                (%result.ok (%tuple.new (%variant.new $json-value null) $next))))
          )
          (else (nop)))
        (call $json.parse-number $b $p))))

  (func $json.parse
    (param $source $string)
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%let $b (%string.bytes $source)
      (block
        (result (%inst $result (ref $json-value) i32))
        (if
          (i32.gt_u (%bytes.len $b) 65536)
          (then (return (%result.err 65536)))
          (else (nop)))
        (%result.do
          ($parsed (call $json.parse-value $b (call $json.space $b 0) 0))
          (%let ($value $next) $parsed
            (%let $end (call $json.space $b $next)
              (if
                (result (%inst $result (ref $json-value) i32))
                (= $end (%bytes.len $b))
                (then (%result.ok $value))
                (else (%result.err $end))))
          ))))
  )

  (func $json.null
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value null))

  (func $json.boolean
    (param $value i32)
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value boolean (!= $value 0)))

  (func $json.from-string
    (param $value $string)
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value string $value))

  (func $json.array
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value array (ref.null $Items)))

  (func $json.object
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value object (ref.null $Members)))

  (func $json.number
    (param $lexeme $string)
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%result.let ($value)
      (call $json.parse $lexeme)
      (%match $value
        (case null () (%result.err 0))
        (case boolean ($x) (%result.err 0))
        (case number ($x) (%result.ok $value))
        (case string ($x) (%result.err 0))
        (case array ($x) (%result.err 0))
        (case object ($x) (%result.err 0))))
  )

  (func $json.kind
    (param $value (ref $json-value))
    (result (ref $json-kind))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%variant.new $json-kind null))
      (case boolean ($x) (%variant.new $json-kind boolean))
      (case number ($x) (%variant.new $json-kind number))
      (case string ($x) (%variant.new $json-kind string))
      (case array ($x) (%variant.new $json-kind array))
      (case object ($x) (%variant.new $json-kind object))))

  (func $json.as-boolean
    (param $value (ref $json-value))
    (result (%inst $option i32))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%option.none))
      (case boolean ($x) (%option.some $x))
      (case number ($x) (%option.none))
      (case string ($x) (%option.none))
      (case array ($x) (%option.none))
      (case object ($x) (%option.none))))

  (func $json.as-number
    (param $value (ref $json-value))
    (result (%inst $option $string))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%option.none))
      (case boolean ($x) (%option.none))
      (case number ($x) (%option.some $x))
      (case string ($x) (%option.none))
      (case array ($x) (%option.none))
      (case object ($x) (%option.none))))

  (func $json.as-string
    (param $value (ref $json-value))
    (result (%inst $option $string))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%option.none))
      (case boolean ($x) (%option.none))
      (case number ($x) (%option.none))
      (case string ($x) (%option.some $x))
      (case array ($x) (%option.none))
      (case object ($x) (%option.none))))

  (func $json.prepend
    (param $array (ref $json-value))
    (param $item (ref $json-value))
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%match $array
      (case null () (%result.err 0))
      (case boolean ($x) (%result.err 0))
      (case number ($x) (%result.err 0))
      (case string ($x) (%result.err 0))
      (case array ($x)
        (%result.ok (%variant.new $json-value array (struct.new $Items $item $x))))
      (case object ($x) (%result.err 0))))

  (func $json.set
    (param $object (ref $json-value))
    (param $key $string)
    (param $item (ref $json-value))
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%match $object
      (case null () (%result.err 0))
      (case boolean ($x) (%result.err 0))
      (case number ($x) (%result.err 0))
      (case string ($x) (%result.err 0))
      (case array ($x) (%result.err 0))
      (case object ($x)
        (%let $reverse (call $json.reverse-members $x (ref.null $Members))
          (%result.ok
            (%variant.new $json-value object
              (call $json.reverse-members $reverse
                (struct.new $Members $key $item (ref.null $Members))))
          ))))
  )

  (func $json.get
    (param $object (ref $json-value))
    (param $key $string)
    (result (%inst $option (ref $json-value)))
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
          (bind $found (%inst $option (ref $json-value)) (%option.none))
          (result (%inst $option (ref $json-value)))
          (if
            (result (%inst $option (ref $json-value)))
            (ref.is_null $head)
            (then $found)
            (else
              (%let $node (ref.as_non_null $head)
                (%recur $get
                  (struct.get $Members $next $node)
                  (if
                    (result (%inst $option (ref $json-value)))
                    (call $bytes.equal
                      (%string.bytes $key)
                      (%string.bytes (struct.get $Members $key $node)))
                    (then (%option.some (struct.get $Members $value $node)))
                    (else $found))))
            ))))
    ))

  (func $json.at
    (param $array (ref $json-value))
    (param $index i32)
    (result (%inst $option (ref $json-value)))
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
          (result (%inst $option (ref $json-value)))
          (if
            (result (%inst $option (ref $json-value)))
            (ref.is_null $head)
            (then (%option.none))
            (else
              (%let $node (ref.as_non_null $head)
                (if
                  (result (%inst $option (ref $json-value)))
                  (= $index 0)
                  (then (%option.some (struct.get $Items $value $node)))
                  (else (%recur $at (struct.get $Items $next $node) (- $index 1))))
              ))))
      )
      (case object ($x) (%option.none))))

  (func $json.write
    (param $value (ref $json-value))
    (param $out (ref $string-builder))
    (%effect reads writes allocates)
    (%match $value
      (case null () (call $string-builder.text $out (%string.lit "null")))
      (case boolean ($x)
        (call $string-builder.text $out
          (if
            (result $string)
            $x
            (then (%string.lit "true"))
            (else (%string.lit "false"))))
      )
      (case number ($x) (call $string-builder.text $out $x))
      (case string ($x) (call $string-builder.text $out (call $json-string.quote $x)))
      (case array ($x)
        (block
          (call $string-builder.text $out (%string.lit "["))
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
                      (then (call $string-builder.text $out (%string.lit ",")))
                      (else (nop)))
                    (call $json.write (struct.get $Items $value $node) $out)
                    (%recur $write-array (struct.get $Items $next $node) 0))))
            ))
          (call $string-builder.text $out (%string.lit "]"))))
      (case object ($x)
        (block
          (call $string-builder.text $out (%string.lit "{"))
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
                      (then (call $string-builder.text $out (%string.lit ",")))
                      (else (nop)))
                    (call $string-builder.text $out (call $json-string.quote (struct.get $Members $key $node)))
                    (call $string-builder.text $out (%string.lit ":"))
                    (call $json.write (struct.get $Members $value $node) $out)
                    (%recur $write-object (struct.get $Members $next $node) 0))))
            ))
          (call $string-builder.text $out (%string.lit "}"))))
    ))

  (func $json.stringify
    (param $value (ref $json-value))
    (result $string)
    (%effect reads writes allocates)
    (block (result $string)
      ;; A standalone JSON string needs no intermediate builder or second copy.
      (%match $value
        (case null () (nop))
        (case boolean ($x) (nop))
        (case number ($x) (nop))
        (case string ($x) (return (call $json-string.quote $x)))
        (case array ($x) (nop))
        (case object ($x) (nop)))
      (%let $out (call $string-builder.new)
        (block (result $string)
          (call $json.write $value $out)
          (call $string-builder.finish $out)))))
)
