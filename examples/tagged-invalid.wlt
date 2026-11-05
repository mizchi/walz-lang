(module
  ;; This is well typed, but violates the intended "Add has two children" rule.
  (type $Expr (struct
    (field $tag i32) (field $value i64)
    (field $left (ref null $Expr)) (field $right (ref null $Expr))))

  (func $main (export "main") (result i64)
    (%let $node (struct.new $Expr (i32.const 1) (i64.const 0)
      (ref.null $Expr) (ref.null $Expr))
      (if (result i64) (i32.eq (struct.get $Expr $tag (local.get $node)) (i32.const 1))
        (then (struct.get $Expr $value
          (ref.as_non_null (struct.get $Expr $left (local.get $node)))))
        (else (struct.get $Expr $value (local.get $node)))))))
