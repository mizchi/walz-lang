(module
  (type $Cell (struct (field $value (mut i64))))

  ;; The captured reference is immutable; the Cell's field is explicitly mutable.
  (%closure $Increment (%capture $cell (ref $Cell)) (result i64)
    (block (result i64)
      (struct.set $Cell $value (local.get $cell)
        (i64.add (struct.get $Cell $value (local.get $cell)) (i64.const 1)))
      (struct.get $Cell $value (local.get $cell))))

  (func $main (export "main") (result i64)
    (%let $cell (struct.new $Cell (i64.const 40))
      (%let $increment (%closure.new $Increment (local.get $cell))
        (block (result i64)
          (drop (%closure.call $Increment (local.get $increment)))
          (%closure.call $Increment (local.get $increment)))))))
