(module
  (%namespace $list-builder
    (%import "wlt:std" (type $list))
    (%export
      (opaque $Builder)
      (func $new)
      (func $push)
      (func $extend)
      (func $len)
      (func $finish)
      (func $clear))
    (type $Chunk
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (struct (field $value $T) (field $previous (ref null (%inst $Chunk $T))))
    )
    (type $Builder
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (struct
        (field $head (mut (ref null (%inst $Chunk $T))))
        (field $length (mut i32))))
    (type $Storage (%type.params $T) (%where (shared $T) (nonunit $T)) (array (mut $T)))
    (func $new
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (result (ref (%inst $Builder $T)))
      (%effects allocates)
      (struct.new (%inst $Builder $T) (ref.null (%inst $Chunk $T)) (i32.const 0)))

    (func $len
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $b (ref (%inst $Builder $T)))
      (result i32)
      (%effects reads)
      (struct.get (%inst $Builder $T) $length (local.get $b)))

    (func $clear
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $b (ref (%inst $Builder $T)))
      (%effects writes)
      (block
        (struct.set
          (%inst $Builder $T)
          $head
          (local.get $b)
          (ref.null (%inst $Chunk $T)))
        (struct.set (%inst $Builder $T) $length (local.get $b) (i32.const 0))))

    (func $push
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $b (ref (%inst $Builder $T)))
      (param $value $T)
      (%effects reads writes allocates)
      (%let $length (struct.get (%inst $Builder $T) $length (local.get $b))
        (if
          (i32.eq (local.get $length) (i32.const -1))
          (then (unreachable))
          (else
            (block
              (struct.set
                (%inst $Builder $T)
                $head
                (local.get $b)
                (struct.new
                  (%inst $Chunk $T)
                  (local.get $value)
                  (struct.get (%inst $Builder $T) $head (local.get $b))))
              (struct.set
                (%inst $Builder $T)
                $length
                (local.get $b)
                (i32.add (local.get $length) (i32.const 1))))
          ))))

    (func $extend
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $b (ref (%inst $Builder $T)))
      (param $values (%inst $list $T))
      (%effects reads writes allocates)
      (%for $append
        (bind $i i32 (i32.const 0))
        (while (i32.lt_u (local.get $i) (%list.len $T (local.get $values))))
        (step (i32.add (local.get $i) (i32.const 1)))
        (call
          (%inst $push $T)
          (local.get $b)
          (%list.get $T (local.get $values) (local.get $i))))
    )

    (func $copy
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $out (ref (%inst $Storage $T)))
      (param $head (ref (%inst $Chunk $T)))
      (param $length i32)
      (%effects writes)
      (%loop $copy
        (bind $chunk (ref null (%inst $Chunk $T)) (local.get $head))
        (bind $end i32 (local.get $length))
        (if
          (ref.is_null (local.get $chunk))
          (then (nop))
          (else
            (%let $chunk (ref.as_non_null (local.get $chunk))
              (block
                (array.set
                  (%inst $Storage $T)
                  (local.get $out)
                  (i32.sub (local.get $end) (i32.const 1))
                  (struct.get (%inst $Chunk $T) $value (local.get $chunk)))
                (%recur $copy
                  (struct.get (%inst $Chunk $T) $previous (local.get $chunk))
                  (i32.sub (local.get $end) (i32.const 1))))
            ))))
    )

    (func $finish
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $b (ref (%inst $Builder $T)))
      (result (%inst $list $T))
      (%effects reads writes allocates)
      (%let $length (struct.get (%inst $Builder $T) $length (local.get $b))
        (if
          (result (%inst $list $T))
          (i32.eqz (local.get $length))
          (then (%list.new $T))
          (else
            (%let
              (($head
                 (ref.as_non_null (struct.get (%inst $Builder $T) $head (local.get $b))))
               ($out
                 (array.new
                   (%inst $Storage $T)
                   (struct.get (%inst $Chunk $T) $value (local.get $head))
                   (local.get $length))))
              (block
                (result (%inst $list $T))
                (call
                  (%inst $copy $T)
                  (local.get $out)
                  (local.get $head)
                  (local.get $length))
                (%let $snapshot
                  (%list.from-array $T (%inst $Storage $T) (local.get $out))
                  (block
                    (result (%inst $list $T))
                    (call (%inst $clear $T) (local.get $b))
                    (local.get $snapshot))))
            ))))
    )))
