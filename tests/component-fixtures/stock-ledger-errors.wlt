(module
  (%include "../../examples/apps/stock-ledger/program.wlt")
  (func $main
    (export "main")
    (result (%result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let $state
      (call $stock.new (i32.const 1))
      (%match
        (%result i64 (ref $stock.Error))
        (result (%result i32 i32))
        (call $stock.add (local.get $state) (%text.lit "max") (i64.const -1))
        (case err ($error) (%result.err i32 i32 (i32.const 999)))
        (case ok
          ($n)
          (%result.let
            ()
            (%result %unit i32)
            i32
            (call $app.process (local.get $state) (%text.lit "ADD\tmax\t1"))
            (%result.let
              ()
              (%result %unit i32)
              i32
              (call $app.process (local.get $state) (%text.lit "ADD\tnew\t1"))
              (%result.ok i32 i32 (i32.const 0))
            )
          )
        )
      )
    )
  )
)
