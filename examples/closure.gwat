(module
  ;; The declaration names the closure type, captures, and call signature.
  (closure $Adder
    (capture $base i64)
    (param $x i64)
    (result i64)
    (i64.add (local.get $base) (local.get $x)))

  ;; The environment is a GC object and can outlive this invocation.
  (func $make-adder (param $base i64) (result (ref $Adder))
    (closure.new $Adder (local.get $base)))

  (func $main (export "main") (result i64)
    (let $add40 (call $make-adder (i64.const 40))
      (closure.call $Adder (local.get $add40) (i64.const 2)))))
