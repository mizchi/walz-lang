(module
  (type $Cell (struct (field $value (mut i64))))
  (type $Expr (%variant (literal i64) (add (ref $Expr) (ref $Expr))))
  (%closure $Add (%capture $base i64) (param $value i64) (result i64)
    (i64.add (local.get $base) (local.get $value)))
  (%closure $Increment (%capture $cell (ref $Cell)) (result i64)
    (block (result i64)
      (struct.set $Cell $value (local.get $cell)
        (i64.add (struct.get $Cell $value (local.get $cell)) (i64.const 1)))
      (struct.get $Cell $value (local.get $cell))))
  (func $make (param $base i64) (result (ref $Add))
    (%closure.new $Add (local.get $base)))
  (func $eval (param $expr (ref $Expr)) (result i64)
    (%match $Expr (result i64) (local.get $expr)
      (case literal ($value) (local.get $value))
      (case add ($left $right)
        (i64.add (call $eval (local.get $left)) (call $eval (local.get $right))))))

  (%test "escaping immutable closure"
    (%let $f (call $make (i64.const 40))
      (i64.eq (%closure.call $Add (local.get $f) (i64.const 2)) (i64.const 42))))

  (%test "captured mutable reference"
    (%let $cell (struct.new $Cell (i64.const 40))
      (%let $f (%closure.new $Increment (local.get $cell))
        (block (result i32)
          (drop (%closure.call $Increment (local.get $f)))
          (i64.eq (%closure.call $Increment (local.get $f)) (i64.const 42))))))

  (%test "recursive GC enum"
    (i64.eq (call $eval
      (%variant.new $Expr add
        (%variant.new $Expr literal (i64.const 20))
        (%variant.new $Expr literal (i64.const 22))))
      (i64.const 42)))

  (%test "GC reference identity"
    (%let $cell (struct.new $Cell (i64.const 42))
      (i32.and (ref.eq (local.get $cell) (local.get $cell))
        (i32.eqz (ref.eq (local.get $cell) (struct.new $Cell (i64.const 42))))))))
