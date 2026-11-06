(module
  (%import "wlt:std"
    (type $result)
    (type $pair))
  (%include "program.wlt")

  ;; 成功値は配送された行数。I/O errは伝播し、取消しの終了確認はtask scopeが担当する。
  (func $main
    (export "main")
    (result (%inst $result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let (($ledger (call $stock.new (i32.const 4096)))
           ($initial (%inst $pair (ref $stock.State) i32)
             (%pair.new (local.get $ledger) (i32.const 0)))
           ($input (call $read-lines)))
      (%result.let ($final-state)
        (%stream.fold-result
          (%move $input)
          (local.get $initial)
          ($state $line)
          (%let ($ledger $count) (local.get $state)
            (%result.let ()
              (call $app.process (local.get $ledger) (local.get $line))
              (%result.ok
                (%pair.new (local.get $ledger)
                  (i32.add (local.get $count) (i32.const 1)))))))
        (%let ($ledger $count) (local.get $final-state)
          (%result.ok (local.get $count)))))))
