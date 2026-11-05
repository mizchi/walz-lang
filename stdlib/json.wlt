(module
  (namespace $json
    (import "builder.wlt"
      (type $builder.Builder)
      (func $builder.finish)
      (func $builder.json)
      (func $builder.new)
      (func $builder.s64)
      (func $builder.text)
      (func $builder.u64))
    (export
      (opaque $Object)
      (func $new)
      (func $string)
      (func $u64)
      (func $s64)
      (func $boolean)
      (func $object)
      (func $finish))
    (type $Object
      (struct (field $builder (ref $builder.Builder)) (field $first (mut i32))))
    (func $new (result (ref $Object)) (effects reads writes allocates)
      (let $b (call $builder.new)
        (block
          (result (ref $Object))
          (call $builder.text (local.get $b) (text.lit "{"))
          (struct.new $Object (local.get $b) (i32.const 1))))
    )

    (func $key (param $o (ref $Object)) (param $key text)
      (effects reads writes allocates)
      (block
        (if
          (i32.eqz (struct.get $Object $first (local.get $o)))
          (then
            (call $builder.text
              (struct.get $Object $builder (local.get $o))
              (text.lit ",")))
          (else (block)))
        (struct.set $Object $first (local.get $o) (i32.const 0))
        (call $builder.json
          (struct.get $Object $builder (local.get $o))
          (local.get $key))
        (call $builder.text (struct.get $Object $builder (local.get $o)) (text.lit ":"))))

    (func $string (param $o (ref $Object)) (param $key text) (param $value text)
      (effects reads writes allocates)
      (block
        (call $key (local.get $o) (local.get $key))
        (call $builder.json
          (struct.get $Object $builder (local.get $o))
          (local.get $value))))

    (func $u64 (param $o (ref $Object)) (param $key text) (param $value i64)
      (effects reads writes allocates)
      (block
        (call $key (local.get $o) (local.get $key))
        (call $builder.u64
          (struct.get $Object $builder (local.get $o))
          (local.get $value))))

    (func $s64 (param $o (ref $Object)) (param $key text) (param $value i64)
      (effects reads writes allocates)
      (block
        (call $key (local.get $o) (local.get $key))
        (call $builder.s64
          (struct.get $Object $builder (local.get $o))
          (local.get $value))))

    (func $boolean (param $o (ref $Object)) (param $key text) (param $value i32)
      (effects reads writes allocates)
      (block
        (call $key (local.get $o) (local.get $key))
        (call $builder.text
          (struct.get $Object $builder (local.get $o))
          (if
            (result text)
            (local.get $value)
            (then (text.lit "true"))
            (else (text.lit "false"))))
      ))

    (func $finish (param $o (ref $Object)) (result text)
      (effects reads writes allocates)
      (let $b (struct.get $Object $builder (local.get $o))
        (block
          (result text)
          (call $builder.text (local.get $b) (text.lit "}"))
          (let $result (call $builder.finish (local.get $b))
            (block
              (result text)
              (call $builder.text (local.get $b) (text.lit "{"))
              (struct.set $Object $first (local.get $o) (i32.const 1))
              (local.get $result))))
      ))

    (func $object (param $o (ref $Object)) (param $key text)
      (param $value (ref $Object)) (effects reads writes allocates)
      ;; Drain the child before touching the parent: aliases cannot form cycles.
      (let $value (call $finish (local.get $value))
        (block
          (call $key (local.get $o) (local.get $key))
          (call $builder.text
            (struct.get $Object $builder (local.get $o))
            (local.get $value))))
    )))
