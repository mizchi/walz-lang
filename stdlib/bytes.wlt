(module
  (namespace $buffer
    (export
      (func $concat)
      (func $equal)
      (func $find-byte)
      (func $find)
      (func $split-once)
      (func $parse-u64))
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
    ;; Return the length when absent or start is out of range. byte is unsigned.
    (func $find-byte (param $b bytes) (param $start i32) (param $byte i32) (result i32)
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
              (i32.eq (bytes.get (local.get $b) (local.get $i)) (local.get $byte))
              (then (local.get $i))
              (else (recur $scan (i32.add (local.get $i) (i32.const 1))))
            ))))
    )

    (func $matches (param $b bytes) (param $needle bytes) (param $start i32)
      (result i32) (effects pure)
      (loop $scan
        (bind $i i32 (i32.const 0))
        (result i32)
        (if
          (result i32)
          (i32.eq (local.get $i) (bytes.len (local.get $needle)))
          (then (i32.const 1))
          (else
            (if
              (result i32)
              (i32.ne
                (bytes.get (local.get $b) (i32.add (local.get $start) (local.get $i)))
                (bytes.get (local.get $needle) (local.get $i)))
              (then (i32.const 0))
              (else (recur $scan (i32.add (local.get $i) (i32.const 1))))
            ))))
    )

    (func $find (param $b bytes) (param $needle bytes) (param $start i32)
      (result (option i32)) (effects pure)
      (if
        (result (option i32))
        (i32.gt_u (local.get $start) (bytes.len (local.get $b)))
        (then (option.none))
        (else
          (if
            (result (option i32))
            (i32.gt_u
              (bytes.len (local.get $needle))
              (i32.sub (bytes.len (local.get $b)) (local.get $start)))
            (then (option.none))
            (else
              (let $last
                (i32.sub (bytes.len (local.get $b)) (bytes.len (local.get $needle)))
                (loop $scan
                  (bind $i i32 (local.get $start))
                  (result (option i32))
                  (if
                    (result (option i32))
                    (call $matches (local.get $b) (local.get $needle) (local.get $i))
                    (then (option.some (local.get $i)))
                    (else
                      (if
                        (result (option i32))
                        (i32.eq (local.get $i) (local.get $last))
                        (then (option.none))
                        (else (recur $scan (i32.add (local.get $i) (i32.const 1))))
                      ))))
              ))))
      ))

    (func $split-once (param $b bytes) (param $needle bytes)
      (result (option (pair bytes bytes))) (effects pure)
      (match (call $find (local.get $b) (local.get $needle) (i32.const 0))
        (case none () (option.none))
        (case some ($i)
          (let $next (i32.add (local.get $i) (bytes.len (local.get $needle)))
            (option.some
              (pair.new
                (bytes.slice (local.get $b) (i32.const 0) (local.get $i))
                (bytes.slice
                  (local.get $b)
                  (local.get $next)
                  (i32.sub (bytes.len (local.get $b)) (local.get $next))))
            ))))
    )
    ;; ASCII decimal over a checked range; test the cutoff before multiplication.
    (func $parse-u64 (param $b bytes) (param $start i32) (param $length i32)
      (param $limit i64) (result (option i64)) (effects pure)
      (if
        (result (option i64))
        (i32.or
          (i32.eqz (local.get $length))
          (i32.gt_u (local.get $start) (bytes.len (local.get $b))))
        (then (option.none))
        (else
          (if
            (result (option i64))
            (i32.gt_u
              (local.get $length)
              (i32.sub (bytes.len (local.get $b)) (local.get $start)))
            (then (option.none))
            (else
              (let $cutoff (i64.div_u (local.get $limit) (i64.const 10))
                (let $last (i32.wrap_i64 (i64.rem_u (local.get $limit) (i64.const 10)))
                  (let $end (i32.add (local.get $start) (local.get $length))
                    (loop $digits
                      (bind $i i32 (local.get $start))
                      (bind $n i64 (i64.const 0))
                      (result (option i64))
                      (if
                        (result (option i64))
                        (i32.eq (local.get $i) (local.get $end))
                        (then (option.some (local.get $n)))
                        (else
                          (let $d
                            (i32.sub (bytes.get (local.get $b) (local.get $i)) (i32.const 48))
                            (if
                              (result (option i64))
                              (i32.or
                                (i32.gt_u (local.get $d) (i32.const 9))
                                (i32.or
                                  (i64.gt_u (local.get $n) (local.get $cutoff))
                                  (i32.and
                                    (i64.eq (local.get $n) (local.get $cutoff))
                                    (i32.gt_u (local.get $d) (local.get $last))))
                              )
                              (then (option.none))
                              (else
                                (recur $digits
                                  (i32.add (local.get $i) (i32.const 1))
                                  (i64.add
                                    (i64.mul (local.get $n) (i64.const 10))
                                    (i64.extend_i32_u (local.get $d))))
                              ))))
                      ))))
              ))))
      ))
    (type $Array (array (mut i32)))
    (func $concat (param $a bytes) (param $b bytes) (result bytes)
      (effects reads writes allocates)
      (let $n (i32.add (bytes.len (local.get $a)) (bytes.len (local.get $b)))
        (if
          (result bytes)
          (i32.lt_u (local.get $n) (bytes.len (local.get $a)))
          (then (unreachable))
          (else
            (let $out (array.new $Array (i32.const 0) (local.get $n))
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
                              (i32.sub (local.get $i) (bytes.len (local.get $a))))
                          )))
                      (recur $copy (i32.add (local.get $i) (i32.const 1))))
                  ))))
          ))))
  ))
