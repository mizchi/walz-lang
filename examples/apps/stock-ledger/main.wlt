(module
  (%import "wlt:std" (type $result))
  (%include "program.wlt")
  ;; 成功値は0。I/O errは伝播し、取消しの終了確認はtask scopeが担当する。
  (func $main
    (export "main")
    (result (%inst $result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let
      (($initial (call $stock.new (i32.const 4096)))
       ($input (call $read-lines)))
      (%result.let ($unused-state)
        (%stream.fold-result
          (%move $input)
          (local.get $initial)
          ($state $line)
          (%result.let ()
            (call $app.process (local.get $state) (local.get $line))
            (%result.ok (local.get $state))))

        (%result.ok (i32.const 0))))
  ))
