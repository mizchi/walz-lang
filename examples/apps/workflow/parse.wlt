(module
  (%use "./domain.wlt" (type $Command $domain.Command))
  (%use "./lex.wlt"
    (func $equal $lex.equal)
    (func $key $lex.key)
    (func $number $lex.number)
    (func $tab $lex.tab))
  (%use "wlt:core" (type $bytes) (type $option) (type $string))
  (%export (func $command))
  (func $fields
    (param $b $bytes)
    (result (ref $domain.Command))
    (%effect allocates)
    (%let $first (call $lex.tab (local.get $b) (i32.const 0))
      (%let $op (%bytes.slice (local.get $b) (i32.const 0) (local.get $first))
        (%let $start (i32.add (local.get $first) (i32.const 1))
          (if
            (result (ref $domain.Command))
            (call $lex.equal (local.get $op) (%string.bytes (%string.lit "PLAN")))
            (then
              (%match (%inst $option i64) (result (ref $domain.Command))
                (call $lex.number (local.get $b) (local.get $start))
                (case none () (%variant.new $domain.Command invalid))
                (case some ($workers)
                  (if
                    (result (ref $domain.Command))
                    (i32.or
                      (i64.eqz (local.get $workers))
                      (i64.gt_u (local.get $workers) (i64.const 32)))
                    (then (%variant.new $domain.Command invalid))
                    (else
                      (%variant.new $domain.Command plan
                        (i32.wrap_i64 (local.get $workers))))
                  ))))
            (else
              (%let $second (call $lex.tab (local.get $b) (local.get $start))
                (%match (%inst $option $string) (result (ref $domain.Command))
                  (call $lex.key (local.get $b) (local.get $start) (local.get $second))
                  (case none () (%variant.new $domain.Command invalid))
                  (case some ($id)
                    (if
                      (result (ref $domain.Command))
                      (call $lex.equal
                        (local.get $op)
                        (%string.bytes (%string.lit "TASK")))
                      (then
                        (%match (%inst $option i64) (result (ref $domain.Command))
                          (call $lex.number
                            (local.get $b)
                            (i32.add (local.get $second) (i32.const 1)))
                          (case none () (%variant.new $domain.Command invalid))
                          (case some ($duration)
                            (%variant.new $domain.Command task
                              (local.get $id)
                              (local.get $duration))))
                      )
                      (else
                        (if
                          (result (ref $domain.Command))
                          (i32.and
                            (call $lex.equal
                              (local.get $op)
                              (%string.bytes (%string.lit "AFTER")))
                            (i32.eq
                              (call $lex.tab
                                (local.get $b)
                                (i32.add (local.get $second) (i32.const 1)))
                              (%bytes.len (local.get $b))))
                          (then
                            (%match (%inst $option $string)
                              (result (ref $domain.Command))
                              (call $lex.key
                                (local.get $b)
                                (i32.add (local.get $second) (i32.const 1))
                                (%bytes.len (local.get $b)))
                              (case none () (%variant.new $domain.Command invalid))
                              (case some ($dependency)
                                (%variant.new $domain.Command after
                                  (local.get $id)
                                  (local.get $dependency))))
                          )
                          (else (%variant.new $domain.Command invalid))))
                    ))))
            ))))
    ))

  (func $command
    (param $line $string)
    (result (ref $domain.Command))
    (%effect allocates)
    (%let $b (%string.bytes (local.get $line))
      (if
        (result (ref $domain.Command))
        (i32.eqz (%bytes.len (local.get $b)))
        (then (%variant.new $domain.Command skip))
        (else
          (if
            (result (ref $domain.Command))
            (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 35))
            (then (%variant.new $domain.Command skip))
            (else (call $fields (local.get $b))))
        ))))
)
