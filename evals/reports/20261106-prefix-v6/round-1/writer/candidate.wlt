(module
  (%include "program.wlt")
  (%type.alias $FoldState (%pair (ref $stock.State) i32))
  (func $main (export "main") (result (%result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let $initial $FoldState
      (%pair.new (call $stock.new (i32.const 4096)) (i32.const 0))
      (%result.let ($final)
        (%stream.fold-result
          (call $read-lines)
          (local.get $initial)
          ($state $line)
          (%let ($stock $count) (local.get $state)
            (%result.let ()
              (call $app.process (local.get $stock) (local.get $line))
              (%result.ok
                (%pair.new (local.get $stock)
                  (i32.add (local.get $count) (i32.const 1)))))))
        (%let ($stock $count) (local.get $final)
          (%result.ok (local.get $count)))))))
