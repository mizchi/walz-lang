(module
  (%use "wlt:core" (func $json.stringify) (func $json.from-string))
  (%use "wlt:core"
    (type $option)
    (type $tuple)
    (type $string))
  (%use "../stdlib/builder.wlt" (type $string-builder))
  (%use "../stdlib/json.wlt" (type $json-object))
  (%use "../stdlib/bytes.wlt"
    (func $bytes.equal)
    (func $bytes.find)
    (func $bytes.find-byte)
    (func $bytes.parse-u64))
  (%use "../stdlib/builder.wlt"
    (func $string-builder.finish)
    (func $string-builder.new)
    (func $string-builder.text))
  (%use "../stdlib/json.wlt"
    (func $json.object-builder.boolean)
    (func $json.object-builder.finish)
    (func $json.object-builder.new)
    (func $json.object-builder.object)
    (func $json.object-builder.s64)
    (func $json.object-builder.string)
    (func $json.object-builder.u64))
  (%use "../stdlib/text.wlt"
    (func $string.find)
    (func $string.parse-s64)
    (func $string.parse-u64)
    (func $string.split-once))
  (func $equal (param $a $string) (param $b $string) (result i32)
    (call $bytes.equal (%string.bytes (local.get $a)) (%string.bytes (local.get $b))))

  (func $some (param $v (%inst $option i64)) (param $expected i64) (result i32)
    (%match (%inst $option i64) (result i32)
      (local.get $v)
      (case none () (i32.const 0))
      (case some ($n) (i64.eq (local.get $n) (local.get $expected))))
  )

  (func $none (param $v (%inst $option i64)) (result i32)
    (%match (%inst $option i64) (result i32)
      (local.get $v)
      (case none () (i32.const 1))
      (case some ($n) (i32.const 0))))
  (%test "byte search is bounded and compares unsigned octets"
    (%let $b (%string.bytes (%string.lit "a\t🦀\t"))
      (i32.and
        (i32.eq
          (call $bytes.find-byte (local.get $b) (i32.const 2) (i32.const 9))
          (i32.const 6))
        (i32.and
          (i32.eq
            (call $bytes.find-byte (local.get $b) (i32.const -1) (i32.const 9))
            (i32.const 7))
          (i32.eq
            (call $bytes.find-byte (local.get $b) (i32.const 0) (i32.const 265))
            (i32.const 7))))
    ))
  (%test "substring search handles overlap and the empty needle at the end"
    (i32.and
      (%match (%inst $option i32) (result i32)
        (call $bytes.find
          (%string.bytes (%string.lit "aaab"))
          (%string.bytes (%string.lit "aab"))
          (i32.const 0))
        (case none () (i32.const 0))
        (case some ($i) (i32.eq (local.get $i) (i32.const 1))))
      (i32.and
        (%match (%inst $option i32) (result i32)
          (call $string.find (%string.lit "🦀") (%string.lit "") (i32.const 4))
          (case none () (i32.const 0))
          (case some ($i) (i32.eq (local.get $i) (i32.const 4))))
        (%match (%inst $option i32) (result i32)
          (call $string.find (%string.lit "🦀") (%string.lit "") (i32.const 1))
          (case none () (i32.const 1))
          (case some ($i) (i32.const 0))))
    ))
  (%test "split once preserves empty fields and Unicode"
    (i32.and
      (%match (%inst $option (%inst $tuple $string $string)) (result i32)
        (call $string.split-once (%string.lit "🦀::日本語::") (%string.lit "::"))
        (case none () (i32.const 0))
        (case some ($parts)
          (%let ($a $b) (local.get $parts)
            (i32.and
              (call $equal (local.get $a) (%string.lit "🦀"))
              (call $equal (local.get $b) (%string.lit "日本語::"))))
        ))
      (%match (%inst $option (%inst $tuple $string $string)) (result i32)
        (call $string.split-once (%string.lit "x") (%string.lit ""))
        (case none () (i32.const 0))
        (case some ($parts)
          (%let ($a $b) (local.get $parts)
            (i32.and
              (call $equal (local.get $a) (%string.lit ""))
              (call $equal (local.get $b) (%string.lit "x"))))
        ))))
  (%test "bounded decimal rejects invalid ranges, empty fields and overflow"
    (%let $b (%string.bytes (%string.lit "x4294967295!"))
      (i32.and
        (call $some
          (call $bytes.parse-u64
            (local.get $b)
            (i32.const 1)
            (i32.const 10)
            (i64.const 4294967295))
          (i64.const 4294967295))
        (i32.and
          (call $none
            (call $bytes.parse-u64
              (local.get $b)
              (i32.const 1)
              (i32.const 10)
              (i64.const 4294967294)))
          (i32.and
            (call $none
              (call $bytes.parse-u64
                (local.get $b)
                (i32.const -1)
                (i32.const 2)
                (i64.const -1)))
            (call $none
              (call $bytes.parse-u64
                (local.get $b)
                (i32.const 1)
                (i32.const -1)
                (i64.const -1))))
        ))))
  (%test "unsigned decimal covers all 64 bits without wrapping"
    (i32.and
      (call $some
        (call $string.parse-u64 (%string.lit "18446744073709551615"))
        (i64.const -1))
      (i32.and
        (call $none (call $string.parse-u64 (%string.lit "18446744073709551616")))
        (i32.and
          (call $none (call $string.parse-u64 (%string.lit "")))
          (call $none (call $string.parse-u64 (%string.lit "-0"))))
      )))
  (%test "signed decimal accepts the minimum and rejects signs and overflow"
    (i32.and
      (call $some
        (call $string.parse-s64 (%string.lit "-9223372036854775808"))
        (i64.const -9223372036854775808))
      (i32.and
        (call $none (call $string.parse-s64 (%string.lit "9223372036854775808")))
        (i32.and
          (call $none (call $string.parse-s64 (%string.lit "-9223372036854775809")))
          (call $none (call $string.parse-s64 (%string.lit "+1"))))
      )))
  (%test "builder finish drains fragments and preserves previous immutable results"
    (%let $b (call $string-builder.new)
      (block
        (result i32)
        (call $string-builder.text (local.get $b) (%string.lit "日本語"))
        (call $string-builder.text (local.get $b) (%string.lit "🦀"))
        (%let $first (call $string-builder.finish (local.get $b))
          (block
            (result i32)
            (call $string-builder.text (local.get $b) (call $json.stringify (call $json.from-string (%string.lit "\n"))))
            (i32.and
              (call $equal (local.get $first) (%string.lit "日本語🦀"))
              (i32.and
                (call $equal
                  (call $string-builder.finish (local.get $b))
                  (%string.lit "\"\\u000a\""))
                (call $equal (call $string-builder.finish (local.get $b)) (%string.lit ""))))
          ))))
  )
  (%test "JSON object builder escapes keys and values, nests and resets"
    (%let $outer (call $json.object-builder.new)
      (%let $inner (call $json.object-builder.new)
        (block
          (result i32)
          (call $json.object-builder.string (local.get $inner) (%string.lit "\"") (%string.lit "🦀\n"))
          (call $json.object-builder.object (local.get $outer) (%string.lit "nested") (local.get $inner))
          (call $json.object-builder.boolean (local.get $outer) (%string.lit "ok") (i32.const -1))
          (call $json.object-builder.s64 (local.get $outer) (%string.lit "n") (i64.const -1))
          (i32.and
            (call $equal
              (call $json.object-builder.finish (local.get $outer))
              (%string.lit "{\"nested\":{\"\\\"\":\"🦀\\u000a\"},\"ok\":true,\"n\":-1}"))
            (call $equal (call $json.object-builder.finish (local.get $outer)) (%string.lit "{}"))))
      )))
  (%test "JSON child aliases are drained before appending the parent key"
    (%let $o (call $json.object-builder.new)
      (block
        (result i32)
        (call $json.object-builder.u64 (local.get $o) (%string.lit "n") (i64.const 42))
        (call $json.object-builder.object (local.get $o) (%string.lit "previous") (local.get $o))
        (call $equal
          (call $json.object-builder.finish (local.get $o))
          (%string.lit "{\"previous\":{\"n\":42}}"))))
  ))
