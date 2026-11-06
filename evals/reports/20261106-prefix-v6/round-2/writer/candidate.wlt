(module
  (%include "program.wlt")

  ;; read-lines が配送した行を数え、I/O err はそのまま伝播する。
  (func $main
    (export "main")
    (result (%result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let $initial (%pair (ref $stock.State) i32)
      (%pair.new (call $stock.new (i32.const 4096)) (i32.const 0))
      (%result.let ($final) (%result (%pair (ref $stock.State) i32) i32) i32
        (%stream.fold-result %text (%pair (ref $stock.State) i32) i32
          (call $read-lines)
          (local.get $initial)
          ($state $line)
          (%let ($ledger $count) (local.get $state)
            (%result.let () (%result %unit i32) (%pair (ref $stock.State) i32)
              (call $app.process (local.get $ledger) (local.get $line))
              (%result.ok
                (%pair.new
                  (local.get $ledger)
                  (i32.add (local.get $count) (i32.const 1)))))))
        (%let ($ledger $count) (local.get $final)
          (%result.ok (local.get $count)))))))
