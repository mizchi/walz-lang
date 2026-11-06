(module
  (%use "wlt:core" (func $json.stringify) (func $json.from-string))
  (%use "wlt:core"
    (type $bytes)
    (type $option)
    (type $tuple)
    (type $string))
  (%use "../../stdlib/bytes.wlt"
    (func $bytes.find)
    (func $bytes.find-byte)
    (func $bytes.parse-u64))
  (%use "../../stdlib/builder.wlt"
    (func $string-builder.finish)
    (func $string-builder.new)
    (func $string-builder.text))
  (%use "../../stdlib/json.wlt"
    (func $json.object-builder.boolean)
    (func $json.object-builder.finish)
    (func $json.object-builder.new)
    (func $json.object-builder.s64)
    (func $json.object-builder.string)
    (func $json.object-builder.u64))
  (%use "../../stdlib/text.wlt"
    (func $string.concat)
    (func $string.find)
    (func $string.parse-s64)
    (func $string.parse-u64)
    (func $string.s64)
    (func $string.split-once)
    (func $string.u64))
  (func $find-byte (export "find-byte") (param $b $bytes) (param $start i32)
    (param $byte i32) (result i32) (%effect pure)
    (call $bytes.find-byte (local.get $b) (local.get $start) (local.get $byte)))

  (func $find (export "find") (param $b $bytes) (param $needle $bytes) (param $start i32)
    (result (%inst $option i32)) (%effect pure)
    (call $bytes.find (local.get $b) (local.get $needle) (local.get $start)))

  (func $find-text (export "find-text") (param $s $string) (param $needle $string)
    (param $start i32) (result (%inst $option i32)) (%effect pure)
    (call $string.find (local.get $s) (local.get $needle) (local.get $start)))

  (func $split (export "split") (param $s $string) (param $needle $string)
    (result (%inst $option (%inst $tuple $string $string))) (%effect pure)
    (call $string.split-once (local.get $s) (local.get $needle)))

  (func $parse-range (export "parse-range") (param $b $bytes) (param $start i32)
    (param $length i32) (param $limit i64) (result (%inst $option i64)) (%effect pure)
    (call $bytes.parse-u64
      (local.get $b)
      (local.get $start)
      (local.get $length)
      (local.get $limit)))

  (func $parse-unsigned (export "parse-unsigned") (param $s $string) (result (%inst $option i64))
    (%effect pure)
    (call $string.parse-u64 (local.get $s)))

  (func $parse-signed (export "parse-signed") (param $s $string) (result (%inst $option i64))
    (%effect pure)
    (call $string.parse-s64 (local.get $s)))

  (func $repeat (export "repeat") (param $part $string) (param $n i32) (result $string)
    (%effect reads writes allocates)
    (%let $b (call $string-builder.new)
      (%loop $append
        (bind $i i32 (i32.const 0))
        (result $string)
        (if
          (result $string)
          (i32.eq (local.get $i) (local.get $n))
          (then (call $string-builder.finish (local.get $b)))
          (else
            (block
              (result $string)
              (call $string-builder.text (local.get $b) (local.get $part))
              (%recur $append (i32.add (local.get $i) (i32.const 1))))
          ))))
  )

  (func $document (export "document") (param $key $string) (param $value $string)
    (param $n i64) (result $string) (%effect reads writes allocates)
    (%let $o (call $json.object-builder.new)
      (block
        (result $string)
        (call $json.object-builder.string (local.get $o) (local.get $key) (local.get $value))
        (call $json.object-builder.u64 (local.get $o) (%string.lit "unsigned") (local.get $n))
        (call $json.object-builder.s64 (local.get $o) (%string.lit "signed") (local.get $n))
        (call $json.object-builder.boolean (local.get $o) (%string.lit "ok") (i32.const 1))
        (call $json.object-builder.finish (local.get $o))))
  )

  (func $join (export "join") (param $a $string) (param $b $string) (result $string)
    (%effect reads writes allocates)
    (call $string.concat (local.get $a) (local.get $b)))

  (func $quote (export "quote") (param $a $string) (result $string)
    (%effect reads writes allocates)
    (call $json.stringify (call $json.from-string (local.get $a))))

  (func $unsigned (export "unsigned") (param $a i64) (result $string)
    (%effect reads writes allocates)
    (call $string.u64 (local.get $a)))

  (func $signed (export "signed") (param $a i64) (result $string)
    (%effect reads writes allocates)
    (call $string.s64 (local.get $a))))
