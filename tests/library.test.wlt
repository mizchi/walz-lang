(module
  (%import "../stdlib/text.wlt"
    (func $string.concat)
    (func $string.json)
    (func $string.s64)
    (func $string.u64))
  (func $equal (param $a %text) (param $b %text) (result i32)
    (%let $a (%text.bytes (local.get $a))
      (%let $b (%text.bytes (local.get $b))
        (if
          (result i32)
          (i32.ne (%bytes.len (local.get $a)) (%bytes.len (local.get $b)))
          (then (i32.const 0))
          (else
            (%loop $go
              (bind $i i32 (i32.const 0))
              (result i32)
              (if
                (result i32)
                (i32.eq (local.get $i) (%bytes.len (local.get $a)))
                (then (i32.const 1))
                (else
                  (if
                    (result i32)
                    (i32.ne
                      (%bytes.get (local.get $a) (local.get $i))
                      (%bytes.get (local.get $b) (local.get $i)))
                    (then (i32.const 0))
                    (else (%recur $go (i32.add (local.get $i) (i32.const 1))))
                  ))))
          ))))
  )
  (%test "concat preserves Unicode and empty text"
    (i32.and
      (call $equal
        (call $string.concat (%text.lit "日本語") (%text.lit "🦀"))
        (%text.lit "日本語🦀"))
      (call $equal (call $string.concat (%text.lit "") (%text.lit "")) (%text.lit ""))))
  (%test "decimal signed and unsigned boundaries"
    (i32.and
      (call $equal (call $string.u64 (i64.const -1)) (%text.lit "18446744073709551615"))
      (i32.and
        (call $equal
          (call $string.s64 (i64.const -9223372036854775808))
          (%text.lit "-9223372036854775808"))
        (call $equal (call $string.u64 (i64.const 0)) (%text.lit "0"))))
  )
  (%test "JSON escapes quotes, slashes and control bytes but preserves Unicode"
    (i32.and
      (call $equal
        (call $string.json (%text.lit "\"\\\n\t日本語🦀"))
        (%text.lit "\"\\\"\\\\\\u000a\\u0009日本語🦀\""))
      (call $equal (call $string.json (%text.lit "")) (%text.lit "\"\""))))
)
