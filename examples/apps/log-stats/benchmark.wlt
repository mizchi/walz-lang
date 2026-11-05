(module
  (include "parse.wlt")
  (include "stats.wlt")
  (type $Corpus (array text))
  (func $once
    (param $input (ref $Corpus))
    (param $rows i32)
    (result i64)
    (effects reads writes allocates)
    (let $s
      (call $stats.new)
      (loop $scan
        (bind $i i32 (i32.const 0))
        (result i64)
        (if
          (result i64)
          (i32.eq (local.get $i) (local.get $rows))
          (then
            (i64.add
              (i64.extend_i32_u (local.get $rows))
              (i64.add
                (call $stats.total (local.get $s) (variant.new $parse.Level info))
                (i64.add
                  (call $stats.total (local.get $s) (variant.new $parse.Level warn))
                  (call $stats.total (local.get $s) (variant.new $parse.Level error))
                )
              )
            )
          )
          (else
            (if
              (result i64)
              (call $stats.consume
                (local.get $s)
                (call $parse.line
                  (array.get $Corpus
                    (local.get $input)
                    (i32.rem_u (local.get $i) (i32.const 6))
                  )
                )
              )
              (then (recur $scan (i32.add (local.get $i) (i32.const 1))))
              (else (unreachable))
            )
          )
        )
      )
    )
  )
  ;; The same parser and accumulator as main.wlt; deliberately no host/ABI I/O.
  (func $run
    (export "run")
    (param $rows i32)
    (param $rounds i32)
    (result i64)
    (effects reads writes allocates)
    (if
      (result i64)
      (i32.or
        (i32.gt_u (local.get $rows) (i32.const 1000000))
        (i32.or
          (i32.eqz (local.get $rounds))
          (i32.gt_u (local.get $rounds) (i32.const 1000))
        )
      )
      (then (unreachable))
      (else
        (let $input
          (array.new_fixed $Corpus 6
            (text.lit "INFO\t12\trequest started")
            (text.lit "WARN\t40\t遅いリクエスト🦀")
            (text.lit "ERROR\t7\tfailed")
            (text.lit "INFO\t8\trequest completed")
            (text.lit "bad")
            (text.lit "# comment")
          )
          (loop $repeat
            (bind $i i32 (i32.const 0))
            (bind $sum i64 (i64.const 0))
            (result i64)
            (if
              (result i64)
              (i32.eq (local.get $i) (local.get $rounds))
              (then (local.get $sum))
              (else
                (recur $repeat
                  (i32.add (local.get $i) (i32.const 1))
                  (i64.add
                    (local.get $sum)
                    (call $once (local.get $input) (local.get $rows))
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
