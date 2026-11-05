(module
  (closure $Take (capture $x (own i64)) (result i64)
    (match (own i64) (result i64) (move $x)
      (case own ($n) (local.get $n))))

  (test "owned values transfer through an option"
    (let $x (option.some (own i64) (own.new i64 (i64.const 42)))
      (match (option (own i64)) (result i32) (move $x)
        (case none () (i32.const 0))
        (case some ($x)
          (i64.eq (closure.call $Take (closure.new $Take (move $x))) (i64.const 42))))))

  (test "unused owned data is explicitly closed"
    (let $x (own.new unit)
      (block (result i32) (own.close (move $x)) (i32.const 1))))

  (test "loop carries its owner explicitly"
    (i64.eq
      (loop $next (bind $x (own i64) (own.new i64 (i64.const 42)))
                  (bind $n i32 (i32.const 3)) (result i64)
        (if (result i64) (i32.eqz (local.get $n))
          (then (match (own i64) (result i64) (move $x)
            (case own ($value) (local.get $value))))
          (else (recur $next (move $x) (i32.sub (local.get $n) (i32.const 1))))))
      (i64.const 42))))
