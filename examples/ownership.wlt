(module
  (%closure $Take
    (%capture $ticket (%own i64))
    (result i64)
    (%effects pure)
    (%match (%own i64) (result i64) (%move $ticket)
      (case own ($value) (local.get $value))))

  (func $main (export "main") (result i64) (%effects allocates)
    (%let $pair
      (%pair.new (%own i64) i64 (%own.new i64 (i64.const 40)) (i64.const 2))
      (%match (%pair (%own i64) i64) (result i64) (%move $pair)
        (case pair ($ticket $extra)
          (%let $take (%closure.new $Take (%move $ticket))
            (%let $unused (%own.new %unit)
              (block (result i64)
                (%own.close (%move $unused))
                (i64.add (%closure.call $Take (%move $take)) (local.get $extra))))))))))
