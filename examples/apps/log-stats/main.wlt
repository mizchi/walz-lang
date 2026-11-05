(module
  (include "parse.wlt")
  (include "stats.wlt")
  (include "decimal.wlt")
  (include "report.wlt")
  (import "wlt:host/text-io@0.1.0" "read-lines"
    (func $read-lines (result (stream (result text i32))))
  )
  (func $main (export "main") (result (result i32 i32))
    (effects reads writes allocates host suspends)
    (let $initial (call $stats.new)
      (result.let ($state) (result (ref $stats.State) i32) i32
        (stream.fold-result text (ref $stats.State) i32
          (call $read-lines)
          (local.get $initial)
          ($state $line)
          (if
            (result (result (ref $stats.State) i32))
            (call $stats.consume
              (local.get $state)
              (call $parse.line (local.get $line)))
            (then (result.ok (ref $stats.State) i32 (local.get $state)))
            (else (result.err (ref $stats.State) i32 (i32.const 1001))))
        )
        (result.let () (result unit i32) i32
          (call $report.write (local.get $state))
          (result.ok i32 i32 (i32.const 0))))
    )))
