(module
  (%import "wlt:std"
    (type $option)
    (type $pair)
    (type $result)
    (type $stream)
    (type $text)
    (type $unit))
  (%include "program.wlt")
  (func $main
    (export "main")
    (result (%inst $result i32 i32))
    (%effect reads writes allocates host suspends)
    (%let $initial
      (call $stock.new (i32.const 4096))
      (%loop $read
        (bind $lines (%inst $stream (%inst $result $text i32)) (call $read-lines))
        (bind $state (ref $stock.State) (local.get $initial))
        (result (%inst $result i32 i32))
        (%match
          (%inst $pair (%inst $stream (%inst $result $text i32)) (%inst $option (%inst $result $text i32)))
          (result (%inst $result i32 i32))
          (%stream.next (%inst $result $text i32) (%move $lines))
          (case pair
            ($next $item)
            (%match
              (%inst $option (%inst $result $text i32))
              (result (%inst $result i32 i32))
              (local.get $item)
              (case none
                ()
                (block
                  (result (%inst $result i32 i32))
                  (%stream.close (%inst $result $text i32) (%move $next))
                  (%result.ok i32 i32 (i32.const 0))
                )
              )
              (case some
                ($result)
                (%match
                  (%inst $result $text i32)
                  (result (%inst $result i32 i32))
                  (local.get $result)
                  (case err
                    ($code)
                    (block
                      (result (%inst $result i32 i32))
                      (%stream.close (%inst $result $text i32) (%move $next))
                      (%result.err i32 i32 (local.get $code))
                    )
                  )
                  (case ok
                    ($line)
                    (%match
                      (%inst $result $unit i32)
                      (result (%inst $result i32 i32))
                      (call $app.process (local.get $state) (local.get $line))
                      (case err
                        ($code)
                        (block
                          (result (%inst $result i32 i32))
                          (%stream.close (%inst $result $text i32) (%move $next))
                          (%result.err i32 i32 (local.get $code))
                        )
                      )
                      (case ok () (%recur $read (%move $next) (local.get $state)))
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
)
