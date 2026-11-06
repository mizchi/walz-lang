(module
  (%use "wlt:core" (type $bytes) (type $option) (type $string))
  (%use "../../../stdlib/bytes.wlt"
    (func $bytes.equal)
    (func $bytes.find-byte)
    (func $bytes.parse-u64))
  (%export (func $equal) (func $tab) (func $key) (func $number))
  (func $equal
    (param $a $bytes)
    (param $b $bytes)
    (result i32)
    (%effect pure)
    (call $bytes.equal (local.get $a) (local.get $b)))

  (func $tab
    (param $b $bytes)
    (param $start i32)
    (result i32)
    (%effect pure)
    (call $bytes.find-byte (local.get $b) (local.get $start) (i32.const 9)))

  (func $key
    (param $b $bytes)
    (param $start i32)
    (param $end i32)
    (result (%inst $option $string))
    (%effect pure)
    (%let $n (i32.sub (local.get $end) (local.get $start))
      (if
        (result (%inst $option $string))
        (i32.or (i32.eqz (local.get $n)) (i32.gt_u (local.get $n) (i32.const 128)))
        (then (%option.none))
        (else
          (%string.from-bytes
            (%bytes.slice (local.get $b) (local.get $start) (local.get $n))))
      )))

  (func $number
    (param $b $bytes)
    (param $start i32)
    (result (%inst $option i64))
    (%effect pure)
    (call $bytes.parse-u64
      (local.get $b)
      (local.get $start)
      (i32.sub (%bytes.len (local.get $b)) (local.get $start))
      (i64.const 1000000))))
