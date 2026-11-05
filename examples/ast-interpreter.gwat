(module
  ;; Each constructor has only its own payload; binary children cannot be null.
  (type $Expr (variant
    (literal i64)
    (add (ref $Expr) (ref $Expr))
    (multiply (ref $Expr) (ref $Expr))))

  (func $eval (param $expression (ref $Expr)) (result i64)
    (match $Expr (result i64) (local.get $expression)
      (case literal ($value) (local.get $value))
      (case add ($left $right)
        (i64.add (call $eval (local.get $left)) (call $eval (local.get $right))))
      (case multiply ($left $right)
        (i64.mul (call $eval (local.get $left)) (call $eval (local.get $right))))))

  (func $main (export "main") (result i64)
    (call $eval
      (variant.new $Expr add
        (variant.new $Expr multiply
          (variant.new $Expr literal (i64.const 20))
          (variant.new $Expr literal (i64.const 2)))
        (variant.new $Expr literal (i64.const 2))))))
