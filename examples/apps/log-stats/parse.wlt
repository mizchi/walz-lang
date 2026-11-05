(module
  (namespace $parse
    (import "../../../stdlib/bytes.wlt"
      (func $buffer.find-byte)
      (func $buffer.parse-u64))
    (public (type $Line) (type $Level) (func $line))
    (type $Level (variant (info) (warn) (error)))
    (type $Line (variant (skip) (invalid) (info i64) (warn i64) (error i64)))
    (func $tab (param $s bytes) (param $start i32) (result i32) (effects pure)
      (call $buffer.find-byte (local.get $s) (local.get $start) (i32.const 9)))

    (func $level (param $s bytes) (param $end i32) (result i32) (effects pure)
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
                  (i32.eq (bytes.get (local.get $s) (i32.const 3)) (i32.const 79))))
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
                      (i32.eq (bytes.get (local.get $s) (i32.const 3)) (i32.const 78))))
                )
                (then (i32.const 1))
                (else (i32.const -1))))
          ))
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
                        (i32.eq (bytes.get (local.get $s) (i32.const 4)) (i32.const 82))))
                  ))
                (then (i32.const 2))
                (else (i32.const -1))))
            (else (i32.const -1))))
      ))

    (func $duration (param $s bytes) (param $start i32) (param $end i32)
      (result (option i64)) (effects pure)
      (call $buffer.parse-u64
        (local.get $s)
        (local.get $start)
        (i32.sub (local.get $end) (local.get $start))
        (i64.const 4294967295)))

    (func $fields (param $s bytes) (result (ref $Line)) (effects allocates)
      (let $a (call $tab (local.get $s) (i32.const 0))
        (if
          (result (ref $Line))
          (i32.eq (local.get $a) (bytes.len (local.get $s)))
          (then (variant.new $Line invalid))
          (else
            (let $level (call $level (local.get $s) (local.get $a))
              (let $b (call $tab (local.get $s) (i32.add (local.get $a) (i32.const 1)))
                (if
                  (result (ref $Line))
                  (i32.or
                    (i32.lt_s (local.get $level) (i32.const 0))
                    (i32.eq (local.get $b) (bytes.len (local.get $s))))
                  (then (variant.new $Line invalid))
                  (else
                    (match (option i64) (result (ref $Line))
                      (call $duration
                        (local.get $s)
                        (i32.add (local.get $a) (i32.const 1))
                        (local.get $b))
                      (case none () (variant.new $Line invalid))
                      (case some ($ms)
                        (if
                          (result (ref $Line))
                          (i32.eqz (local.get $level))
                          (then (variant.new $Line info (local.get $ms)))
                          (else
                            (if
                              (result (ref $Line))
                              (i32.eq (local.get $level) (i32.const 1))
                              (then (variant.new $Line warn (local.get $ms)))
                              (else (variant.new $Line error (local.get $ms))))
                          ))))
                  ))))
          ))))

    (func $line (param $line text) (result (ref $Line)) (effects allocates)
      (let $s (text.bytes (local.get $line))
        (if
          (result (ref $Line))
          (i32.eqz (bytes.len (local.get $s)))
          (then (variant.new $Line skip))
          (else
            (if
              (result (ref $Line))
              (i32.eq (bytes.get (local.get $s) (i32.const 0)) (i32.const 35))
              (then (variant.new $Line skip))
              (else (call $fields (local.get $s))))
          ))))
  ))
