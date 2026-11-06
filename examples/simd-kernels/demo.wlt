(module
  (%include "kernels.wlt")
  (func $main
    (export "main")
    (result i64)
    (%effect reads writes allocates)
    (%let $xs (call $simd.new-i32 (i32.const 3))
      (block
        (result i64)
        (call $simd.set-i32 (local.get $xs) (i32.const 0) (i32.const 10))
        (call $simd.set-i32 (local.get $xs) (i32.const 1) (i32.const 20))
        (call $simd.set-i32 (local.get $xs) (i32.const 2) (i32.const 12))
        (call $simd.sum-simd (call $simd.pack-i32 (local.get $xs))))
    )))
