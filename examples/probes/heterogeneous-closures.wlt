(module
  ;; Same call signature, distinct nominal closure types.
  (%closure $Add (%capture $n i64) (param $x i64) (result i64)
    (i64.add (local.get $n) (local.get $x)))
  (%closure $Multiply (%capture $n i64) (param $x i64) (result i64)
    (i64.mul (local.get $n) (local.get $x)))
  (func $choose (param $flag i32) (result (ref $Add))
    (if (result (ref $Add)) (local.get $flag)
      (then (%closure.new $Add (i64.const 2)))
      (else (%closure.new $Multiply (i64.const 2))))))
