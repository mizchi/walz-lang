(module
  (namespace $buffer
    (public (func $concat))
    (type $Array (array (mut i32)))
    (func $concat
      (param $a bytes)
      (param $b bytes)
      (result bytes)
      (effects reads writes allocates)
      (let $n
        (i32.add (bytes.len (local.get $a)) (bytes.len (local.get $b)))
        (if
          (result bytes)
          (i32.lt_u (local.get $n) (bytes.len (local.get $a)))
          (then (unreachable))
          (else
            (let $out
              (array.new $Array (i32.const 0) (local.get $n))
              (loop $copy
                (bind $i i32 (i32.const 0))
                (result bytes)
                (if
                  (result bytes)
                  (i32.eq (local.get $i) (local.get $n))
                  (then (bytes.from-array $Array (local.get $out)))
                  (else
                    (block
                      (result bytes)
                      (array.set $Array
                        (local.get $out)
                        (local.get $i)
                        (if
                          (result i32)
                          (i32.lt_u (local.get $i) (bytes.len (local.get $a)))
                          (then (bytes.get (local.get $a) (local.get $i)))
                          (else
                            (bytes.get
                              (local.get $b)
                              (i32.sub (local.get $i) (bytes.len (local.get $a)))
                            )
                          )
                        )
                      )
                      (recur $copy (i32.add (local.get $i) (i32.const 1)))
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
