(module
  (%import "wlt:std"
    (type $option)
    (type $pair)
    (type $text))
  (%import "../stdlib/builder.wlt" (type $builder.Builder))
  (%import "../stdlib/json.wlt" (type $json.Object))
  (%import "../stdlib/bytes.wlt"
    (func $buffer.equal)
    (func $buffer.find)
    (func $buffer.find-byte)
    (func $buffer.parse-u64))
  (%import "../stdlib/builder.wlt"
    (func $builder.finish)
    (func $builder.json)
    (func $builder.new)
    (func $builder.text))
  (%import "../stdlib/json.wlt"
    (func $json.boolean)
    (func $json.finish)
    (func $json.new)
    (func $json.object)
    (func $json.s64)
    (func $json.string)
    (func $json.u64))
  (%import "../stdlib/text.wlt"
    (func $string.find)
    (func $string.parse-s64)
    (func $string.parse-u64)
    (func $string.split-once))
  (func $equal (param $a $text) (param $b $text) (result i32)
    (call $buffer.equal (%text.bytes (local.get $a)) (%text.bytes (local.get $b))))

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
    (%let $b (%text.bytes (%text.lit "a\t🦀\t"))
      (i32.and
        (i32.eq
          (call $buffer.find-byte (local.get $b) (i32.const 2) (i32.const 9))
          (i32.const 6))
        (i32.and
          (i32.eq
            (call $buffer.find-byte (local.get $b) (i32.const -1) (i32.const 9))
            (i32.const 7))
          (i32.eq
            (call $buffer.find-byte (local.get $b) (i32.const 0) (i32.const 265))
            (i32.const 7))))
    ))
  (%test "substring search handles overlap and the empty needle at the end"
    (i32.and
      (%match (%inst $option i32) (result i32)
        (call $buffer.find
          (%text.bytes (%text.lit "aaab"))
          (%text.bytes (%text.lit "aab"))
          (i32.const 0))
        (case none () (i32.const 0))
        (case some ($i) (i32.eq (local.get $i) (i32.const 1))))
      (i32.and
        (%match (%inst $option i32) (result i32)
          (call $string.find (%text.lit "🦀") (%text.lit "") (i32.const 4))
          (case none () (i32.const 0))
          (case some ($i) (i32.eq (local.get $i) (i32.const 4))))
        (%match (%inst $option i32) (result i32)
          (call $string.find (%text.lit "🦀") (%text.lit "") (i32.const 1))
          (case none () (i32.const 1))
          (case some ($i) (i32.const 0))))
    ))
  (%test "split once preserves empty fields and Unicode"
    (i32.and
      (%match (%inst $option (%inst $pair $text $text)) (result i32)
        (call $string.split-once (%text.lit "🦀::日本語::") (%text.lit "::"))
        (case none () (i32.const 0))
        (case some ($parts)
          (%let ($a $b) (local.get $parts)
            (i32.and
              (call $equal (local.get $a) (%text.lit "🦀"))
              (call $equal (local.get $b) (%text.lit "日本語::"))))
        ))
      (%match (%inst $option (%inst $pair $text $text)) (result i32)
        (call $string.split-once (%text.lit "x") (%text.lit ""))
        (case none () (i32.const 0))
        (case some ($parts)
          (%let ($a $b) (local.get $parts)
            (i32.and
              (call $equal (local.get $a) (%text.lit ""))
              (call $equal (local.get $b) (%text.lit "x"))))
        ))))
  (%test "bounded decimal rejects invalid ranges, empty fields and overflow"
    (%let $b (%text.bytes (%text.lit "x4294967295!"))
      (i32.and
        (call $some
          (call $buffer.parse-u64
            (local.get $b)
            (i32.const 1)
            (i32.const 10)
            (i64.const 4294967295))
          (i64.const 4294967295))
        (i32.and
          (call $none
            (call $buffer.parse-u64
              (local.get $b)
              (i32.const 1)
              (i32.const 10)
              (i64.const 4294967294)))
          (i32.and
            (call $none
              (call $buffer.parse-u64
                (local.get $b)
                (i32.const -1)
                (i32.const 2)
                (i64.const -1)))
            (call $none
              (call $buffer.parse-u64
                (local.get $b)
                (i32.const 1)
                (i32.const -1)
                (i64.const -1))))
        ))))
  (%test "unsigned decimal covers all 64 bits without wrapping"
    (i32.and
      (call $some
        (call $string.parse-u64 (%text.lit "18446744073709551615"))
        (i64.const -1))
      (i32.and
        (call $none (call $string.parse-u64 (%text.lit "18446744073709551616")))
        (i32.and
          (call $none (call $string.parse-u64 (%text.lit "")))
          (call $none (call $string.parse-u64 (%text.lit "-0"))))
      )))
  (%test "signed decimal accepts the minimum and rejects signs and overflow"
    (i32.and
      (call $some
        (call $string.parse-s64 (%text.lit "-9223372036854775808"))
        (i64.const -9223372036854775808))
      (i32.and
        (call $none (call $string.parse-s64 (%text.lit "9223372036854775808")))
        (i32.and
          (call $none (call $string.parse-s64 (%text.lit "-9223372036854775809")))
          (call $none (call $string.parse-s64 (%text.lit "+1"))))
      )))
  (%test "builder finish drains fragments and preserves previous immutable results"
    (%let $b (call $builder.new)
      (block
        (result i32)
        (call $builder.text (local.get $b) (%text.lit "日本語"))
        (call $builder.text (local.get $b) (%text.lit "🦀"))
        (%let $first (call $builder.finish (local.get $b))
          (block
            (result i32)
            (call $builder.json (local.get $b) (%text.lit "\n"))
            (i32.and
              (call $equal (local.get $first) (%text.lit "日本語🦀"))
              (i32.and
                (call $equal
                  (call $builder.finish (local.get $b))
                  (%text.lit "\"\\u000a\""))
                (call $equal (call $builder.finish (local.get $b)) (%text.lit ""))))
          ))))
  )
  (%test "JSON object builder escapes keys and values, nests and resets"
    (%let $outer (call $json.new)
      (%let $inner (call $json.new)
        (block
          (result i32)
          (call $json.string (local.get $inner) (%text.lit "\"") (%text.lit "🦀\n"))
          (call $json.object (local.get $outer) (%text.lit "nested") (local.get $inner))
          (call $json.boolean (local.get $outer) (%text.lit "ok") (i32.const -1))
          (call $json.s64 (local.get $outer) (%text.lit "n") (i64.const -1))
          (i32.and
            (call $equal
              (call $json.finish (local.get $outer))
              (%text.lit "{\"nested\":{\"\\\"\":\"🦀\\u000a\"},\"ok\":true,\"n\":-1}"))
            (call $equal (call $json.finish (local.get $outer)) (%text.lit "{}"))))
      )))
  (%test "JSON child aliases are drained before appending the parent key"
    (%let $o (call $json.new)
      (block
        (result i32)
        (call $json.u64 (local.get $o) (%text.lit "n") (i64.const 42))
        (call $json.object (local.get $o) (%text.lit "previous") (local.get $o))
        (call $equal
          (call $json.finish (local.get $o))
          (%text.lit "{\"previous\":{\"n\":42}}"))))
  ))
