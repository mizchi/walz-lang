(module
  (type $Cell (struct (field $n (mut i64))))
  (type $Box (struct (field $n i64)))
  (tag $Error (param (ref $Box)))

  (%test "for expands to loop and branch"
    (%let $cell (struct.new $Cell (i64.const 0))
      (block (result i32)
        (%for $walk (bind $i i64 (i64.const 0))
          (while (i64.lt_s (local.get $i) (i64.const 4)))
          (step (i64.add (local.get $i) (i64.const 1)))
          (struct.set $Cell $n (local.get $cell)
            (i64.add (struct.get $Cell $n (local.get $cell)) (local.get $i))))
        (i64.eq (struct.get $Cell $n (local.get $cell)) (i64.const 6)))))

  (%test "native exception carries GC payload"
    (i64.eq
      (%try (result i64) (do (throw $Error (struct.new $Box (i64.const 42))))
        (catch $Error ($box) (struct.get $Box $n (local.get $box))))
      (i64.const 42)))

  (%test "if is a standard Wasm result expression"
    (i64.eq
      (i64.add (if (result i64) (i32.const 1)
        (then (i64.const 40)) (else (i64.const 0))) (i64.const 2))
      (i64.const 42))))
