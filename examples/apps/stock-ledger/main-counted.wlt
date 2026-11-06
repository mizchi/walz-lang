(module
  (%use "wlt:core" (type $tuple) (type $result) (type $string))
  (%include "program.wlt")
  ;; 透明なaliasで役割を区別する。カウントとI/Oエラーは検査上どちらもi32。
  ;; 状態は台帳参照と、I/O処理に成功した配送行数。i32で折り返す。
  (%type.alias $LineCount i32)
  (%type.alias $IoError i32)
  (%type.alias $ScanState (%inst $tuple (ref $stock.State) $LineCount))
  (func $process-counted-line
    (param $scan-state $ScanState)
    (param $line $string)
    (result (%inst $result $ScanState $IoError))
    (%effect reads writes allocates host suspends)
    (%let ($ledger $line-count) (local.get $scan-state)
      ;; 空行・コメント・業務エラーも、I/Oに成功すれば1行と数える。
      (%result.let ()
        (call $app.process (local.get $ledger) (local.get $line))
        (%result.ok
          (%tuple.new
            (local.get $ledger)
            (i32.add (local.get $line-count) (i32.const 1))))
      )))
  ;; 成功値は最終行数。I/O errは伝播し、取消しはtask scopeが担当する。
  (func $main
    (export "main")
    (result (%inst $result $LineCount $IoError))
    (%effect reads writes allocates host suspends)
    (%let
      (($ledger (call $stock.new (i32.const 4096)))
       ($initial-scan-state $ScanState (%tuple.new (local.get $ledger) (i32.const 0)))
       ($input (call $read-lines)))
      (%result.let ($final-scan-state)
        (%stream.fold-result
          (%move $input)
          (local.get $initial-scan-state)
          ($scan-state $line)
          (call $process-counted-line (local.get $scan-state) (local.get $line)))

        (%let ($unused-ledger $final-line-count) (local.get $final-scan-state)
          (%result.ok (local.get $final-line-count))))
    )))
