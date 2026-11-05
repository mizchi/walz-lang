(module
  (type $List
    (struct (field $head i64) (field $tail (ref null $List))))

  (func $sum (param $xs (ref null $List)) (result i64)
    (if (result i64) (ref.is_null (local.get $xs))
      (then (i64.const 0))
      (else
        (let $node (ref.as_non_null (local.get $xs))
          (i64.add (struct.get $List $head (local.get $node))
            (call $sum (struct.get $List $tail (local.get $node))))))))

  (func $main (export "main") (result i64)
    (call $sum
      (struct.new $List (i64.const 10)
        (struct.new $List (i64.const 20)
          (struct.new $List (i64.const 12) (ref.null $List)))))))
