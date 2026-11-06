(module
  (%import "wlt:std" (type $list) (type $bytes))
  (%include "kernels.wlt")
  (func $ints
    (param $xs (%inst $list i32))
    (result (ref $simd.Ints))
    (%effects writes allocates)
    (%let $out (call $simd.new-i32 (%list.len i32 (local.get $xs)))
      (block
        (result (ref $simd.Ints))
        (%for $copy
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%list.len i32 (local.get $xs))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (call $simd.set-i32
            (local.get $out)
            (local.get $i)
            (%list.get i32 (local.get $xs) (local.get $i))))
        (local.get $out))))

  (func $floats
    (param $xs (%inst $list f32))
    (result (ref $simd.Floats))
    (%effects writes allocates)
    (%let $out (call $simd.new-f32 (%list.len f32 (local.get $xs)))
      (block
        (result (ref $simd.Floats))
        (%for $copy
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%list.len f32 (local.get $xs))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (call $simd.set-f32
            (local.get $out)
            (local.get $i)
            (%list.get f32 (local.get $xs) (local.get $i))))
        (local.get $out))))

  (func $bytes
    (param $xs $bytes)
    (result (ref $simd.Ints))
    (%effects writes allocates)
    (%let $out (call $simd.new-i32 (%bytes.len (local.get $xs)))
      (block
        (result (ref $simd.Ints))
        (%for $copy
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%bytes.len (local.get $xs))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (call $simd.set-i32
            (local.get $out)
            (local.get $i)
            (%bytes.get (local.get $xs) (local.get $i))))
        (local.get $out))))

  (func $sum-scalar
    (export "sum-scalar")
    (param $xs (%inst $list i32))
    (result i64)
    (%effects reads writes allocates)
    (call $simd.sum-scalar (call $ints (local.get $xs))))

  (func $sum-simd
    (export "sum-simd")
    (param $xs (%inst $list i32))
    (result i64)
    (%effects reads writes allocates)
    (call $simd.sum-simd (call $simd.pack-i32 (call $ints (local.get $xs))))
  )

  (func $dot-scalar
    (export "dot-scalar")
    (param $a (%inst $list f32))
    (param $b (%inst $list f32))
    (result f32)
    (%effects reads writes allocates)
    (call $simd.dot-scalar (call $floats (local.get $a)) (call $floats (local.get $b))))

  (func $dot-simd
    (export "dot-simd")
    (param $a (%inst $list f32))
    (param $b (%inst $list f32))
    (result f32)
    (%effects reads writes allocates)
    (call $simd.dot-simd
      (call $simd.pack-f32 (call $floats (local.get $a)))
      (call $simd.pack-f32 (call $floats (local.get $b))))
  )

  (func $bitmap-scalar
    (export "bitmap-scalar")
    (param $a (%inst $list i32))
    (param $b (%inst $list i32))
    (result i32)
    (%effects reads writes allocates)
    (call $simd.bitmap-scalar (call $ints (local.get $a)) (call $ints (local.get $b))))

  (func $bitmap-simd
    (export "bitmap-simd")
    (param $a (%inst $list i32))
    (param $b (%inst $list i32))
    (result i32)
    (%effects reads writes allocates)
    (call $simd.bitmap-simd
      (call $simd.pack-i32 (call $ints (local.get $a)))
      (call $simd.pack-i32 (call $ints (local.get $b))))
  )

  (func $index-scalar
    (export "index-scalar")
    (param $xs $bytes)
    (param $needle i32)
    (result i32)
    (%effects reads writes allocates)
    (call $simd.bytes-scalar (call $bytes (local.get $xs)) (local.get $needle)))

  (func $index-simd
    (export "index-simd")
    (param $xs $bytes)
    (param $needle i32)
    (result i32)
    (%effects reads writes allocates)
    (call $simd.bytes-simd
      (call $simd.pack-bytes (call $bytes (local.get $xs)))
      (local.get $needle))))
