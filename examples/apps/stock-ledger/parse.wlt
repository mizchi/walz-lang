(module
  (%use "wlt:core" (type $bytes) (type $option) (type $string))
  (%use "../../../stdlib/bytes.wlt" (func $bytes.find-byte) (func $bytes.parse-u64))
  (%export (type $Command) (func $command))
  (type $Command
    (%variant (skip) (invalid) (add $string i64) (take $string i64) (show $string)))
  (func $tab
    (param $b $bytes)
    (param $start i32)
    (result i32)
    (%effect pure)
    (call $bytes.find-byte (local.get $b) (local.get $start) (i32.const 9)))

  (func $op
    (param $b $bytes)
    (param $end i32)
    (result i32)
    (%effect pure)
    (if
      (result i32)
      (i32.eq (local.get $end) (i32.const 3))
      (then
        (if
          (result i32)
          (i32.and
            (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 65))
            (i32.and
              (i32.eq (%bytes.get (local.get $b) (i32.const 1)) (i32.const 68))
              (i32.eq (%bytes.get (local.get $b) (i32.const 2)) (i32.const 68))))
          (then (i32.const 1))
          (else (i32.const 0))))
      (else
        (if
          (result i32)
          (i32.eq (local.get $end) (i32.const 4))
          (then
            (if
              (result i32)
              (i32.and
                (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 84))
                (i32.and
                  (i32.eq (%bytes.get (local.get $b) (i32.const 1)) (i32.const 65))
                  (i32.and
                    (i32.eq (%bytes.get (local.get $b) (i32.const 2)) (i32.const 75))
                    (i32.eq (%bytes.get (local.get $b) (i32.const 3)) (i32.const 69))))
              )
              (then (i32.const 2))
              (else
                (if
                  (result i32)
                  (i32.and
                    (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 83))
                    (i32.and
                      (i32.eq (%bytes.get (local.get $b) (i32.const 1)) (i32.const 72))
                      (i32.and
                        (i32.eq
                          (%bytes.get (local.get $b) (i32.const 2))
                          (i32.const 79))
                        (i32.eq
                          (%bytes.get (local.get $b) (i32.const 3))
                          (i32.const 87))))
                  )
                  (then (i32.const 3))
                  (else (i32.const 0))))
            ))
          (else (i32.const 0))))
    ))

  (func $number
    (param $b $bytes)
    (param $start i32)
    (result (%inst $option i64))
    (%effect pure)
    (call $bytes.parse-u64
      (local.get $b)
      (local.get $start)
      (i32.sub (%bytes.len (local.get $b)) (local.get $start))
      (i64.const 4294967295)))

  (func $key
    (param $b $bytes)
    (param $start i32)
    (param $end i32)
    (result (%inst $option $string))
    (%effect pure)
    (%let $len (i32.sub (local.get $end) (local.get $start))
      (if
        (result (%inst $option $string))
        (i32.or (i32.eqz (local.get $len)) (i32.gt_u (local.get $len) (i32.const 256)))
        (then (%option.none $string))
        (else
          (%string.from-bytes
            (%bytes.slice (local.get $b) (local.get $start) (local.get $len))))
      )))

  (func $fields
    (param $b $bytes)
    (result (ref $Command))
    (%effect allocates)
    (%let $first (call $tab (local.get $b) (i32.const 0))
      (%let $op (call $op (local.get $b) (local.get $first))
        (if
          (result (ref $Command))
          (i32.eqz (local.get $op))
          (then (%variant.new $Command invalid))
          (else
            (%let $end
              (call $tab (local.get $b) (i32.add (local.get $first) (i32.const 1)))
              (if
                (result (ref $Command))
                (if
                  (result i32)
                  (i32.eq (local.get $op) (i32.const 3))
                  (then (i32.ne (local.get $end) (%bytes.len (local.get $b))))
                  (else
                    (i32.or
                      (i32.eq (local.get $end) (%bytes.len (local.get $b)))
                      (i32.ne
                        (call $tab
                          (local.get $b)
                          (i32.add (local.get $end) (i32.const 1)))
                        (%bytes.len (local.get $b))))
                  ))
                (then (%variant.new $Command invalid))
                (else
                  (%match (%inst $option $string) (result (ref $Command))
                    (call $key
                      (local.get $b)
                      (i32.add (local.get $first) (i32.const 1))
                      (local.get $end))
                    (case none () (%variant.new $Command invalid))
                    (case some ($key)
                      (if
                        (result (ref $Command))
                        (i32.eq (local.get $op) (i32.const 3))
                        (then (%variant.new $Command show (local.get $key)))
                        (else
                          (%match (%inst $option i64) (result (ref $Command))
                            (call $number
                              (local.get $b)
                              (i32.add (local.get $end) (i32.const 1)))
                            (case none () (%variant.new $Command invalid))
                            (case some ($n)
                              (if
                                (result (ref $Command))
                                (i32.eq (local.get $op) (i32.const 1))
                                (then
                                  (%variant.new $Command add
                                    (local.get $key)
                                    (local.get $n)))
                                (else
                                  (%variant.new $Command take
                                    (local.get $key)
                                    (local.get $n))))
                            ))))
                    ))))
            ))))
    ))

  (func $command
    (param $line $string)
    (result (ref $Command))
    (%effect allocates)
    (%let $b (%string.bytes (local.get $line))
      (if
        (result (ref $Command))
        (i32.eqz (%bytes.len (local.get $b)))
        (then (%variant.new $Command skip))
        (else
          (if
            (result (ref $Command))
            (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 35))
            (then (%variant.new $Command skip))
            (else (call $fields (local.get $b))))
        ))))
)
