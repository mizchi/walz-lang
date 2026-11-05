(module
  (%import "../../stdlib/bytes.wlt"
    (func $buffer.find)
    (func $buffer.find-byte)
    (func $buffer.parse-u64))
  (%import "../../stdlib/builder.wlt"
    (func $builder.finish)
    (func $builder.new)
    (func $builder.text))
  (%import "../../stdlib/json.wlt"
    (func $json.boolean)
    (func $json.finish)
    (func $json.new)
    (func $json.s64)
    (func $json.string)
    (func $json.u64))
  (%import "../../stdlib/text.wlt"
    (func $string.concat)
    (func $string.find)
    (func $string.json)
    (func $string.parse-s64)
    (func $string.parse-u64)
    (func $string.s64)
    (func $string.split-once)
    (func $string.u64))
  (func $find-byte (export "find-byte") (param $b %bytes) (param $start i32)
    (param $byte i32) (result i32) (%effects pure)
    (call $buffer.find-byte (local.get $b) (local.get $start) (local.get $byte)))

  (func $find (export "find") (param $b %bytes) (param $needle %bytes) (param $start i32)
    (result (%option i32)) (%effects pure)
    (call $buffer.find (local.get $b) (local.get $needle) (local.get $start)))

  (func $find-text (export "find-text") (param $s %text) (param $needle %text)
    (param $start i32) (result (%option i32)) (%effects pure)
    (call $string.find (local.get $s) (local.get $needle) (local.get $start)))

  (func $split (export "split") (param $s %text) (param $needle %text)
    (result (%option (%pair %text %text))) (%effects pure)
    (call $string.split-once (local.get $s) (local.get $needle)))

  (func $parse-range (export "parse-range") (param $b %bytes) (param $start i32)
    (param $length i32) (param $limit i64) (result (%option i64)) (%effects pure)
    (call $buffer.parse-u64
      (local.get $b)
      (local.get $start)
      (local.get $length)
      (local.get $limit)))

  (func $parse-unsigned (export "parse-unsigned") (param $s %text) (result (%option i64))
    (%effects pure)
    (call $string.parse-u64 (local.get $s)))

  (func $parse-signed (export "parse-signed") (param $s %text) (result (%option i64))
    (%effects pure)
    (call $string.parse-s64 (local.get $s)))

  (func $repeat (export "repeat") (param $part %text) (param $n i32) (result %text)
    (%effects reads writes allocates)
    (%let $b (call $builder.new)
      (%loop $append
        (bind $i i32 (i32.const 0))
        (result %text)
        (if
          (result %text)
          (i32.eq (local.get $i) (local.get $n))
          (then (call $builder.finish (local.get $b)))
          (else
            (block
              (result %text)
              (call $builder.text (local.get $b) (local.get $part))
              (%recur $append (i32.add (local.get $i) (i32.const 1))))
          ))))
  )

  (func $document (export "document") (param $key %text) (param $value %text)
    (param $n i64) (result %text) (%effects reads writes allocates)
    (%let $o (call $json.new)
      (block
        (result %text)
        (call $json.string (local.get $o) (local.get $key) (local.get $value))
        (call $json.u64 (local.get $o) (%text.lit "unsigned") (local.get $n))
        (call $json.s64 (local.get $o) (%text.lit "signed") (local.get $n))
        (call $json.boolean (local.get $o) (%text.lit "ok") (i32.const 1))
        (call $json.finish (local.get $o))))
  )

  (func $join (export "join") (param $a %text) (param $b %text) (result %text)
    (%effects reads writes allocates)
    (call $string.concat (local.get $a) (local.get $b)))

  (func $quote (export "quote") (param $a %text) (result %text)
    (%effects reads writes allocates)
    (call $string.json (local.get $a)))

  (func $unsigned (export "unsigned") (param $a i64) (result %text)
    (%effects reads writes allocates)
    (call $string.u64 (local.get $a)))

  (func $signed (export "signed") (param $a i64) (result %text)
    (%effects reads writes allocates)
    (call $string.s64 (local.get $a))))
