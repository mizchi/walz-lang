(module
  (rec
    (type $S (struct (field $n i32)))
  )
  (func $main (export "main") (result i32)
    (local $__v0 (ref $S))
    (block (result i32) (local.set $__v0 (struct.new $S (i32.const 42))) (struct.get $S 0 (local.get $__v0)))
  )
)
