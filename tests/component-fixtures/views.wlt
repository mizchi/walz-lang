(module
  (%use "wlt:core"
    (type $bytes)
    (type $string)
    (type $future)
    (type $unit)
    (func $bytes-view.new)
    (func $bytes-view.slice)
    (func $bytes-view.get)
    (func $bytes-view.len)
    (func $bytes-view.to-bytes)
    (func $string-view.new)
    (func $string-view.slice)
    (func $string-view.byte-len)
    (func $string-view.bytes)
    (func $string-view.to-string))
  (func $bytes
    (export "bytes")
    (param $input $bytes)
    (param $start i32)
    (param $length i32)
    (param $sub-start i32)
    (param $sub-length i32)
    (result $bytes)
    (%effect allocates)
    (call $bytes-view.to-bytes
      (call $bytes-view.slice
        (call $bytes-view.slice (call $bytes-view.new $input) $start $length)
        $sub-start
        $sub-length)))

  (func $byte
    (export "byte")
    (param $input $bytes)
    (param $start i32)
    (param $length i32)
    (param $index i32)
    (result i32)
    (%effect allocates)
    (call $bytes-view.get
      (call $bytes-view.slice (call $bytes-view.new $input) $start $length)
      $index))

  (func $string
    (export "text")
    (param $input $string)
    (param $start i32)
    (param $length i32)
    (param $sub-start i32)
    (param $sub-length i32)
    (result $string)
    (%effect allocates)
    (call $string-view.to-string
      (call $string-view.slice
        (call $string-view.slice (call $string-view.new $input) $start $length)
        $sub-start
        $sub-length)))

  (func $length
    (export "length")
    (param $input $string)
    (param $start i32)
    (param $length i32)
    (result i32)
    (%effect allocates)
    (%let $v (call $string-view.slice (call $string-view.new $input) $start $length)
      (-
        (call $string-view.byte-len $v)
        (call $bytes-view.len (call $string-view.bytes $v))))
  )

  (func $after
    (export "after")
    (param $input $string)
    (param $start i32)
    (param $length i32)
    (param $pause (%inst $future $unit))
    (result $string)
    (%effect allocates suspends)
    (%let $v (call $string-view.slice (call $string-view.new $input) $start $length)
      (block
        (result $string)
        (%future.await $unit (%move $pause))
        (call $string-view.to-string $v))))
)
