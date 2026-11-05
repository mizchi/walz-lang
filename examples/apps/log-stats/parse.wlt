(module
  (namespace $parse
    (public (type $Line) (type $Level) (func $line))
    (type $Level (variant (info) (warn) (error)))
    (type $Line (variant (skip) (invalid) (info i64) (warn i64) (error i64)))
    (func $tab
      (param $s bytes)
      (param $start i32)
      (result i32)
      (effects pure)
      (loop $find
        (bind $i i32 (local.get $start))
        (result i32)
        (if
          (result i32)
          (i32.ge_u (local.get $i) (bytes.len (local.get $s)))
          (then (bytes.len (local.get $s)))
          (else
            (if
              (result i32)
              (i32.eq (bytes.get (local.get $s) (local.get $i)) (i32.const 9))
              (then (local.get $i))
              (else (recur $find (i32.add (local.get $i) (i32.const 1))))
            )
          )
        )
      )
    )
    (func $level
      (param $s bytes)
      (param $end i32)
      (result i32)
      (effects pure)
      (if
        (result i32)
        (i32.eq (local.get $end) (i32.const 4))
        (then
          (if
            (result i32)
            (i32.and
              (i32.eq (bytes.get (local.get $s) (i32.const 0)) (i32.const 73))
              (i32.and
                (i32.eq (bytes.get (local.get $s) (i32.const 1)) (i32.const 78))
                (i32.and
                  (i32.eq (bytes.get (local.get $s) (i32.const 2)) (i32.const 70))
                  (i32.eq (bytes.get (local.get $s) (i32.const 3)) (i32.const 79))
                )
              )
            )
            (then (i32.const 0))
            (else
              (if
                (result i32)
                (i32.and
                  (i32.eq (bytes.get (local.get $s) (i32.const 0)) (i32.const 87))
                  (i32.and
                    (i32.eq (bytes.get (local.get $s) (i32.const 1)) (i32.const 65))
                    (i32.and
                      (i32.eq (bytes.get (local.get $s) (i32.const 2)) (i32.const 82))
                      (i32.eq (bytes.get (local.get $s) (i32.const 3)) (i32.const 78))
                    )
                  )
                )
                (then (i32.const 1))
                (else (i32.const -1))
              )
            )
          )
        )
        (else
          (if
            (result i32)
            (i32.eq (local.get $end) (i32.const 5))
            (then
              (if
                (result i32)
                (i32.and
                  (i32.eq (bytes.get (local.get $s) (i32.const 0)) (i32.const 69))
                  (i32.and
                    (i32.eq (bytes.get (local.get $s) (i32.const 1)) (i32.const 82))
                    (i32.and
                      (i32.eq (bytes.get (local.get $s) (i32.const 2)) (i32.const 82))
                      (i32.and
                        (i32.eq (bytes.get (local.get $s) (i32.const 3)) (i32.const 79))
                        (i32.eq (bytes.get (local.get $s) (i32.const 4)) (i32.const 82))
                      )
                    )
                  )
                )
                (then (i32.const 2))
                (else (i32.const -1))
              )
            )
            (else (i32.const -1))
          )
        )
      )
    )
    (func $duration
      (param $s bytes)
      (param $start i32)
      (param $end i32)
      (result (option i64))
      (effects pure)
      (if
        (result (option i64))
        (i32.eq (local.get $start) (local.get $end))
        (then (option.none i64))
        (else
          (loop $digits
            (bind $i i32 (local.get $start))
            (bind $n i64 (i64.const 0))
            (result (option i64))
            (if
              (result (option i64))
              (i32.eq (local.get $i) (local.get $end))
              (then (option.some i64 (local.get $n)))
              (else
                (let $d
                  (i32.sub (bytes.get (local.get $s) (local.get $i)) (i32.const 48))
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
    (func $fields
      (param $s bytes)
      (result (ref $Line))
      (effects allocates)
      (let $a
        (call $tab (local.get $s) (i32.const 0))
        (if
          (result (ref $Line))
          (i32.eq (local.get $a) (bytes.len (local.get $s)))
          (then (variant.new $Line invalid))
          (else
            (let $level
              (call $level (local.get $s) (local.get $a))
              (let $b
                (call $tab (local.get $s) (i32.add (local.get $a) (i32.const 1)))
                (if
                  (result (ref $Line))
                  (i32.or
                    (i32.lt_s (local.get $level) (i32.const 0))
                    (i32.eq (local.get $b) (bytes.len (local.get $s)))
                  )
                  (then (variant.new $Line invalid))
                  (else
                    (match
                      (option i64)
                      (result (ref $Line))
                      (call $duration
                        (local.get $s)
                        (i32.add (local.get $a) (i32.const 1))
                        (local.get $b)
                      )
                      (case none () (variant.new $Line invalid))
                      (case some
                        ($ms)
                        (if
                          (result (ref $Line))
                          (i32.eqz (local.get $level))
                          (then (variant.new $Line info (local.get $ms)))
                          (else
                            (if
                              (result (ref $Line))
                              (i32.eq (local.get $level) (i32.const 1))
                              (then (variant.new $Line warn (local.get $ms)))
                              (else (variant.new $Line error (local.get $ms)))
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
    (func $line
      (param $line text)
      (result (ref $Line))
      (effects allocates)
      (let $s
        (text.bytes (local.get $line))
        (if
          (result (ref $Line))
          (i32.eqz (bytes.len (local.get $s)))
          (then (variant.new $Line skip))
          (else
            (if
              (result (ref $Line))
              (i32.eq (bytes.get (local.get $s) (i32.const 0)) (i32.const 35))
              (then (variant.new $Line skip))
              (else (call $fields (local.get $s)))
            )
          )
        )
      )
    )
  )
)
