(module
  (namespace $lex
    (public (func $equal) (func $tab) (func $key) (func $number))
    (func $equal (param $a bytes) (param $b bytes) (result i32) (effects pure)
      (if
        (result i32)
        (i32.ne (bytes.len (local.get $a)) (bytes.len (local.get $b)))
        (then (i32.const 0))
        (else
          (loop $compare
            (bind $i i32 (i32.const 0))
            (result i32)
            (if
              (result i32)
              (i32.eq (local.get $i) (bytes.len (local.get $a)))
              (then (i32.const 1))
              (else
                (if
                  (result i32)
                  (i32.ne
                    (bytes.get (local.get $a) (local.get $i))
                    (bytes.get (local.get $b) (local.get $i)))
                  (then (i32.const 0))
                  (else (recur $compare (i32.add (local.get $i) (i32.const 1))))
                ))))
        )))

    (func $tab (param $b bytes) (param $start i32) (result i32) (effects pure)
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
            ))))
    )

    (func $key (param $b bytes) (param $start i32) (param $end i32)
      (result (option text)) (effects pure)
      (let $n (i32.sub (local.get $end) (local.get $start))
        (if
          (result (option text))
          (i32.or (i32.eqz (local.get $n)) (i32.gt_u (local.get $n) (i32.const 128)))
          (then (option.none))
          (else
            (text.from-bytes
              (bytes.slice (local.get $b) (local.get $start) (local.get $n))))
        )))

    (func $number (param $b bytes) (param $start i32) (result (option i64))
      (effects pure)
      (if
        (result (option i64))
        (i32.ge_u (local.get $start) (bytes.len (local.get $b)))
        (then (option.none))
        (else
          (loop $digits
            (bind $i i32 (local.get $start))
            (bind $n i64 (i64.const 0))
            (result (option i64))
            (if
              (result (option i64))
              (i32.eq (local.get $i) (bytes.len (local.get $b)))
              (then (option.some (local.get $n)))
              (else
                (let $d
                  (i32.sub (bytes.get (local.get $b) (local.get $i)) (i32.const 48))
                  (let $next
                    (i64.add (i64.mul (local.get $n) (i64.const 10)) (i64.extend_i32_u (local.get $d)))
                    (if
                      (result (option i64))
                      (i32.or
                        (i32.gt_u (local.get $d) (i32.const 9))
                        (i64.gt_u (local.get $next) (i64.const 1000000)))
                      (then (option.none))
                      (else
                        (recur $digits
                          (i32.add (local.get $i) (i32.const 1))
                          (local.get $next))))
                  ))))
          ))))
  )
  (namespace $parse
    (public (func $command))
    (func $fields (param $b bytes) (result (ref $domain.Command)) (effects allocates)
      (let $first (call $lex.tab (local.get $b) (i32.const 0))
        (let $op (bytes.slice (local.get $b) (i32.const 0) (local.get $first))
          (let $start (i32.add (local.get $first) (i32.const 1))
            (if
              (result (ref $domain.Command))
              (call $lex.equal (local.get $op) (text.bytes (text.lit "PLAN")))
              (then
                (match (option i64) (result (ref $domain.Command))
                  (call $lex.number (local.get $b) (local.get $start))
                  (case none () (variant.new $domain.Command invalid))
                  (case some ($workers)
                    (if
                      (result (ref $domain.Command))
                      (i32.or
                        (i64.eqz (local.get $workers))
                        (i64.gt_u (local.get $workers) (i64.const 32)))
                      (then (variant.new $domain.Command invalid))
                      (else
                        (variant.new $domain.Command plan
                          (i32.wrap_i64 (local.get $workers))))
                    ))))
              (else
                (let $second (call $lex.tab (local.get $b) (local.get $start))
                  (match (option text) (result (ref $domain.Command))
                    (call $lex.key
                      (local.get $b)
                      (local.get $start)
                      (local.get $second))
                    (case none () (variant.new $domain.Command invalid))
                    (case some ($id)
                      (if
                        (result (ref $domain.Command))
                        (call $lex.equal (local.get $op) (text.bytes (text.lit "TASK")))
                        (then
                          (match (option i64) (result (ref $domain.Command))
                            (call $lex.number
                              (local.get $b)
                              (i32.add (local.get $second) (i32.const 1)))
                            (case none () (variant.new $domain.Command invalid))
                            (case some ($duration)
                              (variant.new $domain.Command task
                                (local.get $id)
                                (local.get $duration))))
                        )
                        (else
                          (if
                            (result (ref $domain.Command))
                            (i32.and
                              (call $lex.equal
                                (local.get $op)
                                (text.bytes (text.lit "AFTER")))
                              (i32.eq
                                (call $lex.tab
                                  (local.get $b)
                                  (i32.add (local.get $second) (i32.const 1)))
                                (bytes.len (local.get $b))))
                            (then
                              (match (option text) (result (ref $domain.Command))
                                (call $lex.key
                                  (local.get $b)
                                  (i32.add (local.get $second) (i32.const 1))
                                  (bytes.len (local.get $b)))
                                (case none () (variant.new $domain.Command invalid))
                                (case some ($dependency)
                                  (variant.new $domain.Command after
                                    (local.get $id)
                                    (local.get $dependency))))
                            )
                            (else (variant.new $domain.Command invalid))))
                      ))))
              ))))
      ))

    (func $command (param $line text) (result (ref $domain.Command)) (effects allocates)
      (let $b (text.bytes (local.get $line))
        (if
          (result (ref $domain.Command))
          (i32.eqz (bytes.len (local.get $b)))
          (then (variant.new $domain.Command skip))
          (else
            (if
              (result (ref $domain.Command))
              (i32.eq (bytes.get (local.get $b) (i32.const 0)) (i32.const 35))
              (then (variant.new $domain.Command skip))
              (else (call $fields (local.get $b))))
          ))))
  ))
