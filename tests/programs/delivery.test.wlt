(module
  (%closure $Await (%capture $future (%future i64)) (result i64) (%effects suspends)
    (%future.await i64 (%move $future)))

  (%test "ready future delivers its value"
    (i64.eq (%future.await i64 (%future.ready i64 (i64.const 42))) (i64.const 42)))

  (%test "EOF returns an owner and repeated EOF stays empty"
    (%match (%pair (%stream %unit) (%option %unit)) (result i32)
      (%stream.next %unit (%stream.empty %unit))
      (case pair ($stream $first)
        (%match (%pair (%stream %unit) (%option %unit)) (result i32)
          (%stream.next %unit (%move $stream))
          (case pair ($end $second)
            (block (result i32)
              (%stream.close %unit (%move $end))
              (i32.and
                (%match (%option %unit) (result i32) (local.get $first)
                  (case none () (i32.const 1)) (case some () (i32.const 0)))
                (%match (%option %unit) (result i32) (local.get $second)
                  (case none () (i32.const 1)) (case some () (i32.const 0))))))))))

  (%test "future capture moves into a once-call closure"
    (%let $future (%future.ready i64 (i64.const 42))
      (%let $await (%closure.new $Await (%move $future))
        (i64.eq (%closure.call $Await (%move $await)) (i64.const 42))))))
