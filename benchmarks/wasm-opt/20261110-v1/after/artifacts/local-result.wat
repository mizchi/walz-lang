(module
  (rec
    (type $__value0 (sub (struct (field i32))))
    (type $__variant_0_0 (sub final $__value0 (struct (field i32) (field i32))))
    (type $__variant_0_1 (sub final $__value0 (struct (field i32) (field i32))))
  )
  (func $main (export "main") (result i32)
    (local $__v0 (ref $__value0))
    (local $__v1 i32)
    (local $__v2 i32)
    (local $__temp0 (ref $__value0))
    (block (result i32) (local.set $__v0 (struct.new $__variant_0_0 (i32.const 0) (i32.const 42))) (block (result i32) (local.set $__temp0 (local.get $__v0)) (if (result i32) (i32.eq (struct.get $__value0 0 (local.get $__temp0)) (i32.const 0)) (then (local.set $__v1 (struct.get $__variant_0_0 1 (ref.cast (ref $__variant_0_0) (local.get $__temp0)))) (local.get $__v1)) (else (if (result i32) (i32.eq (struct.get $__value0 0 (local.get $__temp0)) (i32.const 1)) (then (local.set $__v2 (struct.get $__variant_0_1 1 (ref.cast (ref $__variant_0_1) (local.get $__temp0)))) (local.get $__v2)) (else (unreachable)))))))
  )
)
