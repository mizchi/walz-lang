(module
  (%use "wlt:core"
    (type $bytes)
    (type $future)
    (type $option)
    (type $tuple)
    (type $result)
    (type $stream)
    (type $string))
  (%use "./identity.wlt" (func $bytes $id-bytes) (func $text $id-text))
  (func $bytes
    (export "bytes")
    (param $b $bytes)
    (result $bytes)
    (%effect pure)
    (call $id-bytes (local.get $b)))

  (func $text
    (export "text")
    (param $t $string)
    (result $string)
    (%effect pure)
    (call $id-text (local.get $t)))

  (func $literal
    (export "literal")
    (result $string)
    (%effect pure)
    (%string.lit "日本語🦀"))

  (func $decode
    (export "decode")
    (param $b $bytes)
    (result (%inst $option $string))
    (%effect pure)
    (%string.from-bytes (local.get $b)))

  (func $encode
    (export "encode")
    (param $t $string)
    (result $bytes)
    (%effect pure)
    (%string.bytes (local.get $t)))

  (func $slice
    (export "slice")
    (param $b $bytes)
    (param $start i32)
    (param $len i32)
    (result $bytes)
    (%effect pure)
    (%bytes.slice (local.get $b) (local.get $start) (local.get $len)))

  (func $get
    (export "get")
    (param $b $bytes)
    (param $i i32)
    (result i32)
    (%effect pure)
    (%bytes.get (local.get $b) (local.get $i)))

  (func $len
    (export "length")
    (param $t $string)
    (result i32)
    (%effect pure)
    (%string.byte-len (local.get $t)))

  (func $new
    (export "new-bytes")
    (result $bytes)
    (%effect pure)
    (%bytes.new (i32.const 0) (i32.const 255) (i32.const 256) (i32.const -1)))

  (func $nested
    (export "nested")
    (param $v (%inst $option (%inst $tuple $string $bytes)))
    (result (%inst $option (%inst $tuple $string $bytes)))
    (%effect pure)
    (local.get $v))

  (func $ready
    (export "ready")
    (param $t $string)
    (result (%inst $future $string))
    (%effect pure)
    (%future.ready $string (local.get $t)))

  (func $await
    (export "await")
    (param $f (%inst $future $string))
    (result $string)
    (%effect suspends)
    (%future.await $string (%move $f)))

  (func $stream
    (export "stream")
    (param $b $bytes)
    (result (%inst $stream $bytes))
    (%effect pure)
    (%stream.cons $bytes (local.get $b) (%stream.empty $bytes)))

  (func $first
    (export "first")
    (param $s (%inst $stream $bytes))
    (result (%inst $option $bytes))
    (%effect suspends)
    (%match (%inst $tuple (%inst $stream $bytes) (%inst $option $bytes))
      (result (%inst $option $bytes))
      (%stream.next $bytes (%move $s))
      (case pair ($next $item)
        (block
          (result (%inst $option $bytes))
          (%stream.close $bytes (%move $next))
          (local.get $item))))
  )

  (func $variant
    (export "variant")
    (param $v (%inst $option (%inst $result $string $bytes)))
    (result (%inst $option (%inst $result $string $bytes)))
    (%effect pure)
    (local.get $v))

  (func $wide
    (export "wide")
    (param $a0 $string)
    (param $a1 $string)
    (param $a2 $string)
    (param $a3 $string)
    (param $a4 $string)
    (param $a5 $string)
    (param $a6 $string)
    (param $a7 $string)
    (param $a8 $string)
    (result $string)
    (%effect pure)
    (local.get $a8)))
