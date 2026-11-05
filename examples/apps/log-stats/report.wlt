(module
  (namespace $report
    (import "wlt:host/text-io@0.1.0" "write-line"
      (func $write-line (param $line text) (result (future (result unit i32))))
    )
    (public (func $write))
    (func $bucket
      (param $s (ref $stats.State))
      (param $level (ref $parse.Level))
      (result text)
      (effects reads writes allocates)
      (call $string.concat
        (call $string.concat
          (text.lit "{")
          (call $string.concat
            (call $string.concat
              (text.lit "\"count\":")
              (call $decimal.text (call $stats.count (local.get $s) (local.get $level)))
            )
            (text.lit ",")
          )
        )
        (call $string.concat
          (call $string.concat
            (call $string.concat
              (text.lit "\"total_ms\":")
              (call $decimal.text (call $stats.total (local.get $s) (local.get $level)))
            )
            (text.lit ",")
          )
          (call $string.concat
            (call $string.concat
              (text.lit "\"max_ms\":")
              (call $decimal.text
                (call $stats.maximum (local.get $s) (local.get $level))
              )
            )
            (text.lit "}")
          )
        )
      )
    )
    (func $write
      (param $s (ref $stats.State))
      (result (result unit i32))
      (effects reads writes allocates host suspends)
      (future.await
        (result unit i32)
        (call $write-line
          (call $string.concat
            (call $string.concat
              (call $string.concat
                (text.lit "{")
                (call $string.concat
                  (call $string.concat
                    (text.lit "\"accepted\":")
                    (call $decimal.text (call $stats.accepted (local.get $s)))
                  )
                  (text.lit ",")
                )
              )
              (call $string.concat
                (call $string.concat
                  (text.lit "\"rejected\":")
                  (call $decimal.text (call $stats.rejected (local.get $s)))
                )
                (call $string.concat
                  (text.lit ",")
                  (call $string.concat
                    (text.lit "\"ignored\":")
                    (call $decimal.text (call $stats.ignored (local.get $s)))
                  )
                )
              )
            )
            (call $string.concat
              (call $string.concat
                (text.lit ",\"levels\":{\"INFO\":")
                (call $string.concat
                  (call $bucket (local.get $s) (variant.new $parse.Level info))
                  (text.lit ",\"WARN\":")
                )
              )
              (call $string.concat
                (call $string.concat
                  (call $bucket (local.get $s) (variant.new $parse.Level warn))
                  (text.lit ",\"ERROR\":")
                )
                (call $string.concat
                  (call $bucket (local.get $s) (variant.new $parse.Level error))
                  (text.lit "}}")
                )
              )
            )
          )
        )
      )
    )
  )
)
