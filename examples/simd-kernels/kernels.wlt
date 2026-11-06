(module
  (%export
    (opaque $Ints)
    (opaque $Floats)
    (opaque $I32x4)
    (opaque $F32x4)
    (opaque $Bytes16)
    (func $new-i32)
    (func $new-f32)
    (func $set-i32)
    (func $set-f32)
    (func $pack-i32)
    (func $pack-f32)
    (func $pack-bytes)
    (func $sum-scalar)
    (func $sum-simd)
    (func $dot-scalar)
    (func $dot-simd)
    (func $bitmap-scalar)
    (func $bitmap-simd)
    (func $bytes-scalar)
    (func $bytes-simd))
  (type $Ints (array (mut i32)))
  (type $Floats (array (mut f32)))
  (type $Vectors (array (mut v128)))
  ;; Nominal types keep words, float lanes and byte lanes from being mixed.
  (type $I32x4 (struct (field $length i32) (field $values (ref $Vectors))))
  (type $F32x4 (struct (field $length i32) (field $values (ref $Vectors))))
  (type $Bytes16 (struct (field $length i32) (field $values (ref $Vectors))))
  (func $limit
    (param $n i32)
    (%effect pure)
    (if
      (i32.gt_u (local.get $n) (i32.const 16777216))
      (then (unreachable))
      (else (nop))))

  (func $same-length
    (param $a i32)
    (param $b i32)
    (%effect pure)
    (if (i32.ne (local.get $a) (local.get $b)) (then (unreachable)) (else (nop))))

  (func $new-i32
    (param $n i32)
    (result (ref $Ints))
    (%effect allocates)
    (block
      (result (ref $Ints))
      (call $limit (local.get $n))
      (array.new $Ints (i32.const 0) (local.get $n))))

  (func $new-f32
    (param $n i32)
    (result (ref $Floats))
    (%effect allocates)
    (block
      (result (ref $Floats))
      (call $limit (local.get $n))
      (array.new $Floats (f32.const 0) (local.get $n))))

  (func $set-i32
    (param $xs (ref $Ints))
    (param $i i32)
    (param $x i32)
    (%effect writes)
    (array.set $Ints (local.get $xs) (local.get $i) (local.get $x)))

  (func $set-f32
    (param $xs (ref $Floats))
    (param $i i32)
    (param $x f32)
    (%effect writes)
    (array.set $Floats (local.get $xs) (local.get $i) (local.get $x)))
  ;; Only packing uses zero-extended tail reads. Scalar kernels use exact bounds.
  (func $get-i32
    (param $xs (ref $Ints))
    (param $i i32)
    (result i32)
    (%effect reads)
    (if
      (result i32)
      (i32.lt_u (local.get $i) (array.len (local.get $xs)))
      (then (array.get $Ints (local.get $xs) (local.get $i)))
      (else (i32.const 0))))

  (func $get-f32
    (param $xs (ref $Floats))
    (param $i i32)
    (result f32)
    (%effect reads)
    (if
      (result f32)
      (i32.lt_u (local.get $i) (array.len (local.get $xs)))
      (then (array.get $Floats (local.get $xs) (local.get $i)))
      (else (f32.const 0))))

  (func $load-i32x4
    (param $xs (ref $Ints))
    (param $i i32)
    (result v128)
    (%effect reads)
    (i32x4.replace_lane 3
      (i32x4.replace_lane 2
        (i32x4.replace_lane 1
          (i32x4.replace_lane 0
            (v128.const i32x4 0 0 0 0)
            (call $get-i32 (local.get $xs) (local.get $i)))
          (call $get-i32 (local.get $xs) (i32.add (local.get $i) (i32.const 1))))
        (call $get-i32 (local.get $xs) (i32.add (local.get $i) (i32.const 2))))
      (call $get-i32 (local.get $xs) (i32.add (local.get $i) (i32.const 3))))
  )

  (func $load-f32x4
    (param $xs (ref $Floats))
    (param $i i32)
    (result v128)
    (%effect reads)
    (f32x4.replace_lane 3
      (f32x4.replace_lane 2
        (f32x4.replace_lane 1
          (f32x4.replace_lane 0
            (v128.const f32x4 0 0 0 0)
            (call $get-f32 (local.get $xs) (local.get $i)))
          (call $get-f32 (local.get $xs) (i32.add (local.get $i) (i32.const 1))))
        (call $get-f32 (local.get $xs) (i32.add (local.get $i) (i32.const 2))))
      (call $get-f32 (local.get $xs) (i32.add (local.get $i) (i32.const 3))))
  )

  (func $four-bytes
    (param $xs (ref $Ints))
    (param $i i32)
    (result i32)
    (%effect reads)
    (i32.or
      (i32.and (call $get-i32 (local.get $xs) (local.get $i)) (i32.const 255))
      (i32.or
        (i32.shl
          (i32.and
            (call $get-i32 (local.get $xs) (i32.add (local.get $i) (i32.const 1)))
            (i32.const 255))
          (i32.const 8))
        (i32.or
          (i32.shl
            (i32.and
              (call $get-i32 (local.get $xs) (i32.add (local.get $i) (i32.const 2)))
              (i32.const 255))
            (i32.const 16))
          (i32.shl
            (i32.and
              (call $get-i32 (local.get $xs) (i32.add (local.get $i) (i32.const 3)))
              (i32.const 255))
            (i32.const 24))))
    ))

  (func $load-bytes16
    (param $xs (ref $Ints))
    (param $i i32)
    (result v128)
    (%effect reads)
    (i32x4.replace_lane 3
      (i32x4.replace_lane 2
        (i32x4.replace_lane 1
          (i32x4.replace_lane 0
            (v128.const i32x4 0 0 0 0)
            (call $four-bytes (local.get $xs) (local.get $i)))
          (call $four-bytes (local.get $xs) (i32.add (local.get $i) (i32.const 4))))
        (call $four-bytes (local.get $xs) (i32.add (local.get $i) (i32.const 8))))
      (call $four-bytes (local.get $xs) (i32.add (local.get $i) (i32.const 12))))
  )

  (func $pack-i32
    (param $xs (ref $Ints))
    (result (ref $I32x4))
    (%effect reads writes allocates)
    (%let
      (($n (array.len (local.get $xs)))
       ($count (i32.shr_u (i32.add (local.get $n) (i32.const 3)) (i32.const 2)))
       ($out (array.new $Vectors (v128.const i32x4 0 0 0 0) (local.get $count))))
      (block
        (result (ref $I32x4))
        (%for $pack
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (local.get $count)))
          (step (i32.add (local.get $i) (i32.const 1)))
          (array.set $Vectors
            (local.get $out)
            (local.get $i)
            (call $load-i32x4 (local.get $xs) (i32.shl (local.get $i) (i32.const 2))))
        )
        (struct.new $I32x4 (local.get $n) (local.get $out))))
  )

  (func $pack-f32
    (param $xs (ref $Floats))
    (result (ref $F32x4))
    (%effect reads writes allocates)
    (%let
      (($n (array.len (local.get $xs)))
       ($count (i32.shr_u (i32.add (local.get $n) (i32.const 3)) (i32.const 2)))
       ($out (array.new $Vectors (v128.const i32x4 0 0 0 0) (local.get $count))))
      (block
        (result (ref $F32x4))
        (%for $pack
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (local.get $count)))
          (step (i32.add (local.get $i) (i32.const 1)))
          (array.set $Vectors
            (local.get $out)
            (local.get $i)
            (call $load-f32x4 (local.get $xs) (i32.shl (local.get $i) (i32.const 2))))
        )
        (struct.new $F32x4 (local.get $n) (local.get $out))))
  )

  (func $pack-bytes
    (param $xs (ref $Ints))
    (result (ref $Bytes16))
    (%effect reads writes allocates)
    (%let
      (($n (array.len (local.get $xs)))
       ($count (i32.shr_u (i32.add (local.get $n) (i32.const 15)) (i32.const 4)))
       ($out (array.new $Vectors (v128.const i32x4 0 0 0 0) (local.get $count))))
      (block
        (result (ref $Bytes16))
        (%for $pack
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (local.get $count)))
          (step (i32.add (local.get $i) (i32.const 1)))
          (array.set $Vectors
            (local.get $out)
            (local.get $i)
            (call $load-bytes16 (local.get $xs) (i32.shl (local.get $i) (i32.const 4))))
        )
        (struct.new $Bytes16 (local.get $n) (local.get $out))))
  )
  ;; Signed widening before accumulation prevents i32 overflow.
  (func $sum-scalar
    (param $xs (ref $Ints))
    (result i64)
    (%effect reads)
    (%loop $sum
      (bind $i i32 (i32.const 0))
      (bind $sum i64 (i64.const 0))
      (result i64)
      (if
        (result i64)
        (i32.eq (local.get $i) (array.len (local.get $xs)))
        (then (local.get $sum))
        (else
          (%recur $sum
            (i32.add (local.get $i) (i32.const 1))
            (i64.add
              (local.get $sum)
              (i64.extend_i32_s (array.get $Ints (local.get $xs) (local.get $i))))
          ))))
  )

  (func $sum-simd
    (param $xs (ref $I32x4))
    (result i64)
    (%effect reads)
    (%let $values (struct.get $I32x4 $values (local.get $xs))
      (%loop $sum
        (bind $i i32 (i32.const 0))
        (bind $sum v128 (v128.const i64x2 0 0))
        (result i64)
        (if
          (result i64)
          (i32.eq (local.get $i) (array.len (local.get $values)))
          (then
            (i64.add
              (i64x2.extract_lane 0 (local.get $sum))
              (i64x2.extract_lane 1 (local.get $sum))))
          (else
            (%let $v (array.get $Vectors (local.get $values) (local.get $i))
              (%recur $sum
                (i32.add (local.get $i) (i32.const 1))
                (i64x2.add
                  (local.get $sum)
                  (i64x2.add
                    (i64x2.extend_low_i32x4_s (local.get $v))
                    (i64x2.extend_high_i32x4_s (local.get $v))))
              ))))
      )))

  (func $dot-scalar
    (param $a (ref $Floats))
    (param $b (ref $Floats))
    (result f32)
    (%effect reads)
    (block
      (result f32)
      (call $same-length (array.len (local.get $a)) (array.len (local.get $b)))
      (%loop $dot
        (bind $i i32 (i32.const 0))
        (bind $sum f32 (f32.const 0))
        (result f32)
        (if
          (result f32)
          (i32.eq (local.get $i) (array.len (local.get $a)))
          (then (local.get $sum))
          (else
            (%recur $dot
              (i32.add (local.get $i) (i32.const 1))
              (f32.add
                (local.get $sum)
                (f32.mul
                  (array.get $Floats (local.get $a) (local.get $i))
                  (array.get $Floats (local.get $b) (local.get $i))))
            ))))
    ))

  (func $dot-simd
    (param $a (ref $F32x4))
    (param $b (ref $F32x4))
    (result f32)
    (%effect reads)
    (block
      (result f32)
      (call $same-length
        (struct.get $F32x4 $length (local.get $a))
        (struct.get $F32x4 $length (local.get $b)))
      (%let
        (($a-values (struct.get $F32x4 $values (local.get $a)))
         ($b-values (struct.get $F32x4 $values (local.get $b))))
        (%loop $dot
          (bind $i i32 (i32.const 0))
          (bind $sum v128 (v128.const f32x4 0 0 0 0))
          (result f32)
          (if
            (result f32)
            (i32.eq (local.get $i) (array.len (local.get $a-values)))
            (then
              (f32.add
                (f32.add
                  (f32x4.extract_lane 0 (local.get $sum))
                  (f32x4.extract_lane 1 (local.get $sum)))
                (f32.add
                  (f32x4.extract_lane 2 (local.get $sum))
                  (f32x4.extract_lane 3 (local.get $sum))))
            )
            (else
              (%recur $dot
                (i32.add (local.get $i) (i32.const 1))
                (f32x4.add
                  (local.get $sum)
                  (f32x4.mul
                    (array.get $Vectors (local.get $a-values) (local.get $i))
                    (array.get $Vectors (local.get $b-values) (local.get $i))))
              ))))
      )))

  (func $bitmap-scalar
    (param $a (ref $Ints))
    (param $b (ref $Ints))
    (result i32)
    (%effect reads)
    (block
      (result i32)
      (call $same-length (array.len (local.get $a)) (array.len (local.get $b)))
      (%loop $count
        (bind $i i32 (i32.const 0))
        (bind $sum i32 (i32.const 0))
        (result i32)
        (if
          (result i32)
          (i32.eq (local.get $i) (array.len (local.get $a)))
          (then (local.get $sum))
          (else
            (%recur $count
              (i32.add (local.get $i) (i32.const 1))
              (i32.add
                (local.get $sum)
                (i32.popcnt
                  (i32.and
                    (array.get $Ints (local.get $a) (local.get $i))
                    (array.get $Ints (local.get $b) (local.get $i))))
              ))))
      )))

  (func $bitmap-simd
    (param $a (ref $I32x4))
    (param $b (ref $I32x4))
    (result i32)
    (%effect reads)
    (block
      (result i32)
      (call $same-length
        (struct.get $I32x4 $length (local.get $a))
        (struct.get $I32x4 $length (local.get $b)))
      (%let
        (($a-values (struct.get $I32x4 $values (local.get $a)))
         ($b-values (struct.get $I32x4 $values (local.get $b))))
        (%loop $count
          (bind $i i32 (i32.const 0))
          (bind $sum v128 (v128.const i32x4 0 0 0 0))
          (result i32)
          (if
            (result i32)
            (i32.eq (local.get $i) (array.len (local.get $a-values)))
            (then
              (i32.add
                (i32.add
                  (i32x4.extract_lane 0 (local.get $sum))
                  (i32x4.extract_lane 1 (local.get $sum)))
                (i32.add
                  (i32x4.extract_lane 2 (local.get $sum))
                  (i32x4.extract_lane 3 (local.get $sum))))
            )
            (else
              (%recur $count
                (i32.add (local.get $i) (i32.const 1))
                (i32x4.add
                  (local.get $sum)
                  (i32x4.extadd_pairwise_i16x8_u
                    (i16x8.extadd_pairwise_i8x16_u
                      (i8x16.popcnt
                        (v128.and
                          (array.get $Vectors (local.get $a-values) (local.get $i))
                          (array.get $Vectors (local.get $b-values) (local.get $i))))
                    ))))
            ))))
    ))

  (func $byte-needle
    (param $needle i32)
    (%effect pure)
    (if
      (i32.gt_u (local.get $needle) (i32.const 255))
      (then (unreachable))
      (else (nop))))

  (func $bytes-scalar
    (param $xs (ref $Ints))
    (param $needle i32)
    (result i32)
    (%effect reads)
    (block
      (result i32)
      (call $byte-needle (local.get $needle))
      (%loop $find
        (bind $i i32 (i32.const 0))
        (result i32)
        (if
          (result i32)
          (i32.eq (local.get $i) (array.len (local.get $xs)))
          (then (i32.const -1))
          (else
            (if
              (result i32)
              (i32.eq
                (array.get $Ints (local.get $xs) (local.get $i))
                (local.get $needle))
              (then (local.get $i))
              (else (%recur $find (i32.add (local.get $i) (i32.const 1))))
            ))))
    ))

  (func $bytes-simd
    (param $xs (ref $Bytes16))
    (param $needle i32)
    (result i32)
    (%effect reads)
    (block
      (result i32)
      (call $byte-needle (local.get $needle))
      (%let
        (($values (struct.get $Bytes16 $values (local.get $xs)))
         ($needle-v (i8x16.splat (local.get $needle))))
        (%loop $find
          (bind $i i32 (i32.const 0))
          (result i32)
          (if
            (result i32)
            (i32.eq (local.get $i) (array.len (local.get $values)))
            (then (i32.const -1))
            (else
              (%let $mask
                (i8x16.bitmask (i8x16.eq (array.get $Vectors (local.get $values) (local.get $i)) (local.get $needle-v)))
                (if
                  (result i32)
                  (i32.eqz (local.get $mask))
                  (then (%recur $find (i32.add (local.get $i) (i32.const 1))))
                  (else
                    (%let $at
                      (i32.add (i32.shl (local.get $i) (i32.const 4)) (i32.ctz (local.get $mask)))
                      (if
                        (result i32)
                        (i32.lt_u
                          (local.get $at)
                          (struct.get $Bytes16 $length (local.get $xs)))
                        (then (local.get $at))
                        (else (i32.const -1))))
                  ))))
          ))))
  ))
