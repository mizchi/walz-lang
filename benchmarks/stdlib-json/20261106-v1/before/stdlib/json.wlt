(module
  (%import "wlt:std" (type $result) (type $string) (type $unit))
  (%import "json-value.wlt"
    (type $json-value)
    (func $json-value.number)
    (func $json-value.stringify))
  (%import "builder.wlt"
    (type $string-builder)
    (func $string-builder.finish)
    (func $string-builder.json)
    (func $string-builder.new)
    (func $string-builder.s64)
    (func $string-builder.text)
    (func $string-builder.u64))
  (%export
    (opaque $json-object)
    (func $json-object.new)
    (func $json-object.string)
    (func $json-object.u64)
    (func $json-object.s64)
    (func $json-object.boolean)
    (func $json-object.object)
    (func $json-object.null)
    (func $json-object.number)
    (func $json-object.value)
    (func $json-object.finish))
  (type $json-object
    (struct (field $builder (ref $string-builder)) (field $first (mut i32))))
  (func $json-object.new
    (result (ref $json-object))
    (%effect reads writes allocates)
    (%let $b (call $string-builder.new)
      (block
        (result (ref $json-object))
        (call $string-builder.text $b (%string.lit "{"))
        (struct.new $json-object $b 1))))

  (func $json-object.key
    (param $o (ref $json-object))
    (param $key $string)
    (%effect reads writes allocates)
    (block
      (if
        (i32.eqz (struct.get $json-object $first $o))
        (then
          (call $string-builder.text
            (struct.get $json-object $builder $o)
            (%string.lit ",")))
        (else (block)))
      (struct.set $json-object $first $o 0)
      (call $string-builder.json (struct.get $json-object $builder $o) $key)
      (call $string-builder.text
        (struct.get $json-object $builder $o)
        (%string.lit ":"))))

  (func $json-object.string
    (param $o (ref $json-object))
    (param $key $string)
    (param $value $string)
    (%effect reads writes allocates)
    (block
      (call $json-object.key $o $key)
      (call $string-builder.json (struct.get $json-object $builder $o) $value)))

  (func $json-object.u64
    (param $o (ref $json-object))
    (param $key $string)
    (param $value i64)
    (%effect reads writes allocates)
    (block
      (call $json-object.key $o $key)
      (call $string-builder.u64 (struct.get $json-object $builder $o) $value)))

  (func $json-object.s64
    (param $o (ref $json-object))
    (param $key $string)
    (param $value i64)
    (%effect reads writes allocates)
    (block
      (call $json-object.key $o $key)
      (call $string-builder.s64 (struct.get $json-object $builder $o) $value)))

  (func $json-object.boolean
    (param $o (ref $json-object))
    (param $key $string)
    (param $value i32)
    (%effect reads writes allocates)
    (block
      (call $json-object.key $o $key)
      (call $string-builder.text
        (struct.get $json-object $builder $o)
        (if
          (result $string)
          $value
          (then (%string.lit "true"))
          (else (%string.lit "false"))))
    ))

  (func $json-object.finish
    (param $o (ref $json-object))
    (result $string)
    (%effect reads writes allocates)
    (%let $b (struct.get $json-object $builder $o)
      (block
        (result $string)
        (call $string-builder.text $b (%string.lit "}"))
        (%let $result (call $string-builder.finish $b)
          (block
            (result $string)
            (call $string-builder.text $b (%string.lit "{"))
            (struct.set $json-object $first $o 1)
            $result))))
  )

  (func $json-object.object
    (param $o (ref $json-object))
    (param $key $string)
    (param $value (ref $json-object))
    (%effect reads writes allocates)
    ;; Drain the child before touching the parent: aliases cannot form cycles.
    (%let $value (call $json-object.finish $value)
      (block
        (call $json-object.key $o $key)
        (call $string-builder.text (struct.get $json-object $builder $o) $value))))

  (func $json-object.null
    (param $o (ref $json-object))
    (param $key $string)
    (%effect reads writes allocates)
    (block
      (call $json-object.key $o $key)
      (call $string-builder.text
        (struct.get $json-object $builder $o)
        (%string.lit "null"))))

  (func $json-object.value
    (param $o (ref $json-object))
    (param $key $string)
    (param $value (ref $json-value))
    (%effect reads writes allocates)
    (%let $text (call $json-value.stringify $value)
      (block
        (call $json-object.key $o $key)
        (call $string-builder.text (struct.get $json-object $builder $o) $text))))

  (func $json-object.number
    (param $o (ref $json-object))
    (param $key $string)
    (param $lexeme $string)
    (result (%inst $result $unit i32))
    (%effect reads writes allocates)
    (%result.let ($value)
      (call $json-value.number $lexeme)
      (block
        (result (%inst $result $unit i32))
        (call $json-object.value $o $key $value)
        (%result.ok))))
)
