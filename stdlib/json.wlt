(module
  (%namespace $json
    (%import "wlt:std" (type $result) (type $text) (type $unit))
    (%import "json-value.wlt"
      (type $json-value.Value)
      (func $json-value.number)
      (func $json-value.stringify))
    (%import "builder.wlt"
      (type $builder.Builder)
      (func $builder.finish)
      (func $builder.json)
      (func $builder.new)
      (func $builder.s64)
      (func $builder.text)
      (func $builder.u64))
    (%export
      (opaque $Object)
      (func $new)
      (func $string)
      (func $u64)
      (func $s64)
      (func $boolean)
      (func $object)
      (func $null)
      (func $number)
      (func $value)
      (func $finish))
    (type $Object
      (struct (field $builder (ref $builder.Builder)) (field $first (mut i32))))
    (func $new
      (result (ref $Object))
      (%effect reads writes allocates)
      (%let $b (call $builder.new)
        (block
          (result (ref $Object))
          (call $builder.text $b (%text.lit "{"))
          (struct.new $Object $b 1))))

    (func $key
      (param $o (ref $Object))
      (param $key $text)
      (%effect reads writes allocates)
      (block
        (if
          (i32.eqz (struct.get $Object $first $o))
          (then (call $builder.text (struct.get $Object $builder $o) (%text.lit ",")))
          (else (block)))
        (struct.set $Object $first $o 0)
        (call $builder.json (struct.get $Object $builder $o) $key)
        (call $builder.text (struct.get $Object $builder $o) (%text.lit ":"))))

    (func $string
      (param $o (ref $Object))
      (param $key $text)
      (param $value $text)
      (%effect reads writes allocates)
      (block
        (call $key $o $key)
        (call $builder.json (struct.get $Object $builder $o) $value)))

    (func $u64
      (param $o (ref $Object))
      (param $key $text)
      (param $value i64)
      (%effect reads writes allocates)
      (block
        (call $key $o $key)
        (call $builder.u64 (struct.get $Object $builder $o) $value)))

    (func $s64
      (param $o (ref $Object))
      (param $key $text)
      (param $value i64)
      (%effect reads writes allocates)
      (block
        (call $key $o $key)
        (call $builder.s64 (struct.get $Object $builder $o) $value)))

    (func $boolean
      (param $o (ref $Object))
      (param $key $text)
      (param $value i32)
      (%effect reads writes allocates)
      (block
        (call $key $o $key)
        (call $builder.text
          (struct.get $Object $builder $o)
          (if
            (result $text)
            $value
            (then (%text.lit "true"))
            (else (%text.lit "false"))))
      ))

    (func $finish
      (param $o (ref $Object))
      (result $text)
      (%effect reads writes allocates)
      (%let $b (struct.get $Object $builder $o)
        (block
          (result $text)
          (call $builder.text $b (%text.lit "}"))
          (%let $result (call $builder.finish $b)
            (block
              (result $text)
              (call $builder.text $b (%text.lit "{"))
              (struct.set $Object $first $o 1)
              $result))))
    )

    (func $object
      (param $o (ref $Object))
      (param $key $text)
      (param $value (ref $Object))
      (%effect reads writes allocates)
      ;; Drain the child before touching the parent: aliases cannot form cycles.
      (%let $value (call $finish $value)
        (block
          (call $key $o $key)
          (call $builder.text (struct.get $Object $builder $o) $value))))

    (func $null
      (param $o (ref $Object))
      (param $key $text)
      (%effect reads writes allocates)
      (block
        (call $key $o $key)
        (call $builder.text (struct.get $Object $builder $o) (%text.lit "null"))))

    (func $value
      (param $o (ref $Object))
      (param $key $text)
      (param $value (ref $json-value.Value))
      (%effect reads writes allocates)
      (%let $text (call $json-value.stringify $value)
        (block
          (call $key $o $key)
          (call $builder.text (struct.get $Object $builder $o) $text))))

    (func $number
      (param $o (ref $Object))
      (param $key $text)
      (param $lexeme $text)
      (result (%inst $result $unit i32))
      (%effect reads writes allocates)
      (%result.let ($value)
        (call $json-value.number $lexeme)
        (block
          (result (%inst $result $unit i32))
          (call $value $o $key $value)
          (%result.ok))))
  ))
