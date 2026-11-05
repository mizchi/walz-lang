(module
  (type $Node (struct
    (field $value i64) (field $next (mut (ref null $Node)))))

  (func $main (export "main") (result i64)
    (let $a (struct.new $Node (i64.const 20) (ref.null $Node))
      (let $b (struct.new $Node (i64.const 22) (ref.null $Node))
        (block (result i64)
          (struct.set $Node $next (local.get $a) (local.get $b))
          (struct.set $Node $next (local.get $b) (local.get $a))
          (loop $walk
            (bind $node (ref $Node) (local.get $a))
            (bind $i i32 (i32.const 0))
            (bind $sum i64 (i64.const 0))
            (result i64)
            (if (result i64) (i32.lt_u (local.get $i) (i32.const 2))
              (then (recur $walk
                (ref.as_non_null (struct.get $Node $next (local.get $node)))
                (i32.add (local.get $i) (i32.const 1))
                (i64.add (local.get $sum) (struct.get $Node $value (local.get $node)))))
              (else
                (if (result i64) (ref.eq (local.get $node) (local.get $a))
                  (then (local.get $sum))
                  (else (unreachable)))))))))))
