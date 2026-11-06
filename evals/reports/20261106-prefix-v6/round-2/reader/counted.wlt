(module
  (%include "program.wlt")

  ;; 状態は台帳参照と、I/O処理に成功した配送行数。i32で折り返す。
  (%type.alias $ScanState (%pair (ref $stock.State) i32))

  (func $process-counted-line
    (param $progress $ScanState)
    (param $line %text)
    (result (%result $ScanState i32))
    (%effects reads writes allocates host suspends)
    (%let ($ledger $count) (local.get $progress)
      ;; 空行・コメント・業務エラーも、I/Oに成功すれば1行と数える。
      (%result.let ()
        (call $app.process (local.get $ledger) (local.get $line))
        (%result.ok
          (%pair.new
            (local.get $ledger)
            (i32.add (local.get $count) (i32.const 1)))))))

  ;; 成功値は最終行数。I/O errは伝播し、取消しはtask scopeが担当する。
  (func $main
    (export "main")
    (result (%result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let $initial-state $ScanState
      (%pair.new
        (call $stock.new (i32.const 4096))
        (i32.const 0))
      (%result.let ($final-state)
        (%stream.fold-result
          (call $read-lines)
          (local.get $initial-state)
          ($progress $line)
          (call $process-counted-line
            (local.get $progress)
            (local.get $line)))
        (%let ($final-ledger $final-count) (local.get $final-state)
          (%result.ok (local.get $final-count)))))))
