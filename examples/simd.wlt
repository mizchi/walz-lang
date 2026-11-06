(module
  (func $main (export "main") (result i32) (%effect pure)
    (%let $v v128 (v128.const i32x4 40 0 0 0)
      (i32x4.extract_lane 0
        (i32x4.add (local.get $v) (i32x4.splat (i32.const 2)))))))
