(module
  (namespace $parse
    (public (type $Command) (func $command))
    (type $Command
      (variant (skip) (invalid) (add text i64) (take text i64) (show text))
    )
    (func $tab
      (param $b bytes)
      (param $start i32)
      (result i32)
      (effects pure)
      (loop $scan
        (bind $i i32 (local.get $start))
        (result i32)
        (if
          (result i32)
          (i32.ge_u (local.get $i) (bytes.len (local.get $b)))
          (then (bytes.len (local.get $b)))
          (else
            (if
              (result i32)
              (i32.eq (bytes.get (local.get $b) (local.get $i)) (i32.const 9))
              (then (local.get $i))
              (else (recur $scan (i32.add (local.get $i) (i32.const 1))))
            )
          )
        )
      )
    )
    (func $op
      (param $b bytes)
      (param $end i32)
      (result i32)
      (effects pure)
      (if
        (result i32)
        (i32.eq (local.get $end) (i32.const 3))
        (then
          (if
            (result i32)
            (i32.and
              (i32.eq (bytes.get (local.get $b) (i32.const 0)) (i32.const 65))
              (i32.and
                (i32.eq (bytes.get (local.get $b) (i32.const 1)) (i32.const 68))
                (i32.eq (bytes.get (local.get $b) (i32.const 2)) (i32.const 68))
              )
            )
            (then (i32.const 1))
            (else (i32.const 0))
          )
        )
        (else
          (if
            (result i32)
            (i32.eq (local.get $end) (i32.const 4))
            (then
              (if
                (result i32)
                (i32.and
                  (i32.eq (bytes.get (local.get $b) (i32.const 0)) (i32.const 84))
                  (i32.and
                    (i32.eq (bytes.get (local.get $b) (i32.const 1)) (i32.const 65))
                    (i32.and
                      (i32.eq (bytes.get (local.get $b) (i32.const 2)) (i32.const 75))
                      (i32.eq (bytes.get (local.get $b) (i32.const 3)) (i32.const 69))
                    )
                  )
                )
                (then (i32.const 2))
                (else
                  (if
                    (result i32)
                    (i32.and
                      (i32.eq (bytes.get (local.get $b) (i32.const 0)) (i32.const 83))
                      (i32.and
                        (i32.eq (bytes.get (local.get $b) (i32.const 1)) (i32.const 72))
                        (i32.and
                          (i32.eq
                            (bytes.get (local.get $b) (i32.const 2))
                            (i32.const 79)
                          )
                          (i32.eq
                            (bytes.get (local.get $b) (i32.const 3))
                            (i32.const 87)
                          )
                        )
                      )
                    )
                    (then (i32.const 3))
                    (else (i32.const 0))
                  )
                )
              )
            )
            (else (i32.const 0))
          )
        )
      )
    )
    (func $number
      (param $b bytes)
      (param $start i32)
      (result (option i64))
      (effects pure)
      (if
        (result (option i64))
        (i32.eq (local.get $start) (bytes.len (local.get $b)))
        (then (option.none i64))
        (else
          (loop $digits
            (bind $i i32 (local.get $start))
            (bind $n i64 (i64.const 0))
            (result (option i64))
            (if
              (result (option i64))
              (i32.eq (local.get $i) (bytes.len (local.get $b)))
              (then (option.some i64 (local.get $n)))
              (else
                (let $d
                  (i32.sub (bytes.get (local.get $b) (local.get $i)) (i32.const 48))
                  (if
                    (result (option i64))
                    (i32.or
                      (i32.gt_u (local.get $d) (i32.const 9))
                      (i32.or
                        (i64.gt_u (local.get $n) (i64.const 429496729))
                        (i32.and
                          (i64.eq (local.get $n) (i64.const 429496729))
                          (i32.gt_u (local.get $d) (i32.const 5))
                        )
                      )
                    )
                    (then (option.none i64))
                    (else
                      (recur $digits
                        (i32.add (local.get $i) (i32.const 1))
                        (i64.add
                          (i64.mul (local.get $n) (i64.const 10))
                          (i64.extend_i32_u (local.get $d))
                        )
                      )
                    )
                  )
                )
              )
            )
          )
        )
      )
    )
    (func $key
      (param $b bytes)
      (param $start i32)
      (param $end i32)
      (result (option text))
      (effects pure)
      (let $len
        (i32.sub (local.get $end) (local.get $start))
        (if
          (result (option text))
          (i32.or
            (i32.eqz (local.get $len))
            (i32.gt_u (local.get $len) (i32.const 256))
          )
          (then (option.none text))
          (else
            (text.from-bytes
              (bytes.slice (local.get $b) (local.get $start) (local.get $len))
            )
          )
        )
      )
    )
    (func $fields
      (param $b bytes)
      (result (ref $Command))
      (effects allocates)
      (let $first
        (call $tab (local.get $b) (i32.const 0))
        (let $op
          (call $op (local.get $b) (local.get $first))
          (if
            (result (ref $Command))
            (i32.eqz (local.get $op))
            (then (variant.new $Command invalid))
            (else
              (let $end
                (call $tab (local.get $b) (i32.add (local.get $first) (i32.const 1)))
                (if
                  (result (ref $Command))
                  (if
                    (result i32)
                    (i32.eq (local.get $op) (i32.const 3))
                    (then (i32.ne (local.get $end) (bytes.len (local.get $b))))
                    (else
                      (i32.or
                        (i32.eq (local.get $end) (bytes.len (local.get $b)))
                        (i32.ne
                          (call $tab
                            (local.get $b)
                            (i32.add (local.get $end) (i32.const 1))
                          )
                          (bytes.len (local.get $b))
                        )
                      )
                    )
                  )
                  (then (variant.new $Command invalid))
                  (else
                    (match
                      (option text)
                      (result (ref $Command))
                      (call $key
                        (local.get $b)
                        (i32.add (local.get $first) (i32.const 1))
                        (local.get $end)
                      )
                      (case none () (variant.new $Command invalid))
                      (case some
                        ($key)
                        (if
                          (result (ref $Command))
                          (i32.eq (local.get $op) (i32.const 3))
                          (then (variant.new $Command show (local.get $key)))
                          (else
                            (match
                              (option i64)
                              (result (ref $Command))
                              (call $number
                                (local.get $b)
                                (i32.add (local.get $end) (i32.const 1))
                              )
                              (case none () (variant.new $Command invalid))
                              (case some
                                ($n)
                                (if
                                  (result (ref $Command))
                                  (i32.eq (local.get $op) (i32.const 1))
                                  (then
                                    (variant.new $Command add
                                      (local.get $key)
                                      (local.get $n)
                                    )
                                  )
                                  (else
                                    (variant.new $Command take
                                      (local.get $key)
                                      (local.get $n)
                                    )
                                  )
                                )
                              )
                            )
                          )
                        )
                      )
                    )
                  )
                )
              )
            )
          )
        )
      )
    )
    (func $command
      (param $line text)
      (result (ref $Command))
      (effects allocates)
      (let $b
        (text.bytes (local.get $line))
        (if
          (result (ref $Command))
          (i32.eqz (bytes.len (local.get $b)))
          (then (variant.new $Command skip))
          (else
            (if
              (result (ref $Command))
              (i32.eq (bytes.get (local.get $b) (i32.const 0)) (i32.const 35))
              (then (variant.new $Command skip))
              (else (call $fields (local.get $b)))
            )
          )
        )
      )
    )
  )
)
