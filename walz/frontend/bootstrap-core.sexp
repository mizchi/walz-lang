(module
  (type $Marker
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (struct (field $value $T)))
  (type $PairMarker
    (@type.params $T $E)
    (@where (shared $T $E) (nonunit $T $E))
    (struct (field $first $T) (field $second $E)))
  (@export
    (type $bytes)
    (type $string)
    (type $unit)
    (type $list)
    (type $option)
    (type $result)
    (type $tuple))
  (@type.alias $unit @unit)
  (@type.alias $list (@type.params $T) (ref (@inst $List $T)))
  (@type.alias $option (@type.params $T) (ref (@inst $Option $T)))
  (@type.alias $result (@type.params $T $E) (ref (@inst $Result $T $E)))
  (@type.alias $tuple (@type.params $T $U) (ref (@inst $Tuple $T $U)))
  (@type.alias $bytes (ref $Bytes))
  (@type.alias $string (ref $String))
  (type $List
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub
      (struct (field $length i32) (field $marker (ref null (@inst $Marker $T))))
    ))
  (type $Empty
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub final
      (@inst $List $T)
      (struct (field $length i32) (field $marker (ref null (@inst $Marker $T))))
    ))
  (type $Leaf
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub final
      (@inst $List $T)
      (struct
        (field $length i32)
        (field $marker (ref null (@inst $Marker $T)))
        (field $value $T))))
  (type $Branch
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub final
      (@inst $List $T)
      (struct
        (field $length i32)
        (field $marker (ref null (@inst $Marker $T)))
        (field $left (ref (@inst $List $T)))
        (field $right (ref (@inst $List $T))))
    ))
  (type $Option
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub
      (struct (field $tag i32) (field $marker (ref null (@inst $Marker $T))))
    ))
  (type $None
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub final
      (@inst $Option $T)
      (struct (field $tag i32) (field $marker (ref null (@inst $Marker $T))))
    ))
  (type $Some
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (sub final
      (@inst $Option $T)
      (struct
        (field $tag i32)
        (field $marker (ref null (@inst $Marker $T)))
        (field $value $T))))
  (type $Result
    (@type.params $T $E)
    (@where (shared $T $E) (nonunit $T $E))
    (sub
      (struct (field $tag i32) (field $marker (ref null (@inst $PairMarker $T $E))))
    ))
  (type $Ok
    (@type.params $T $E)
    (@where (shared $T $E) (nonunit $T $E))
    (sub final
      (@inst $Result $T $E)
      (struct
        (field $tag i32)
        (field $marker (ref null (@inst $PairMarker $T $E)))
        (field $value $T))))
  (type $Err
    (@type.params $T $E)
    (@where (shared $T $E) (nonunit $T $E))
    (sub final
      (@inst $Result $T $E)
      (struct
        (field $tag i32)
        (field $marker (ref null (@inst $PairMarker $T $E)))
        (field $value $E))))
  (type $Tuple
    (@type.params $T $U)
    (@where (shared $T $U) (nonunit $T $U))
    (struct (field $first $T) (field $second $U)))
  (type $Bytes
    (struct (field $data (ref (@inst $List i32))))
  )
  (type $String (struct (field $bytes (ref $Bytes))))
  (func $List.empty
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (result (ref (@inst $List $T)))
    (@effect pure)
    (struct.new (@inst $Empty $T) (i32.const 0) (ref.null (@inst $Marker $T))))

  (func $List.len
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $xs (ref (@inst $List $T)))
    (result i32)
    (@effect pure)
    (struct.get (@inst $List $T) $length $xs))

  (func $List.get
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $xs (ref (@inst $List $T)))
    (param $index i32)
    (result $T)
    (@effect pure)
    (if
      (result $T)
      (i32.ge_u $index (call (@inst $List.len $T) $xs))
      (then (unreachable))
      (else
        (if
          (result $T)
          (= (call (@inst $List.len $T) $xs) 1)
          (then
            (struct.get (@inst $Leaf $T) $value (ref.cast (ref (@inst $Leaf $T)) $xs)))
          (else
            (@let
              (($b (ref.cast (ref (@inst $Branch $T)) $xs))
               ($left (struct.get (@inst $Branch $T) $left $b))
               ($len (call (@inst $List.len $T) $left)))
              (if
                (result $T)
                (i32.lt_u $index $len)
                (then (call (@inst $List.get $T) $left $index))
                (else
                  (call
                    (@inst $List.get $T)
                    (struct.get (@inst $Branch $T) $right $b)
                    (- $index $len))))
            ))))
    ))

  (func $List.join
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $a (ref (@inst $List $T)))
    (param $b (ref (@inst $List $T)))
    (result (ref (@inst $List $T)))
    (@effect pure)
    (struct.new (@inst $Branch $T)
      (+ (call (@inst $List.len $T) $a) (call (@inst $List.len $T) $b))
      (ref.null (@inst $Marker $T))
      $a
      $b))

  (func $List.from-array
    (@type.params $T $S)
    (@where (shared $T) (nonunit $T))
    (param $a (ref $S))
    (param $offset i32)
    (param $length i32)
    (result (ref (@inst $List $T)))
    (@effect reads)
    (if
      (result (ref (@inst $List $T)))
      (i32.eqz $length)
      (then (call (@inst $List.empty $T)))
      (else
        (if
          (result (ref (@inst $List $T)))
          (= $length 1)
          (then
            (struct.new (@inst $Leaf $T)
              1
              (ref.null (@inst $Marker $T))
              (@storage.get $S $a $offset)))
          (else
            (@let $half (i32.shr_u $length 1)
              (call
                (@inst $List.join $T)
                (call (@inst $List.from-array $T $S) $a $offset $half)
                (call
                  (@inst $List.from-array $T $S)
                  $a
                  (+ $offset $half)
                  (- $length $half))))
          ))))
  )

  ;; Wider integer storage must be checked before publishing byte values.
  ;; Only the selected range is visited; packed i8 keeps the existing fast path.
  (func $ByteList.from-array
    (@type.params $S)
    (param $a (ref $S))
    (param $offset i32)
    (param $length i32)
    (result (ref (@inst $List i32)))
    (@effect reads)
    (if
      (result (ref (@inst $List i32)))
      (i32.eqz $length)
      (then (call (@inst $List.empty i32)))
      (else
        (if
          (result (ref (@inst $List i32)))
          (= $length 1)
          (then
            (struct.new (@inst $Leaf i32)
              1
              (ref.null (@inst $Marker i32))
              (call $bytes.check (@storage.get $S $a $offset))))
          (else
            (@let $half (i32.shr_u $length 1)
              (call
                (@inst $List.join i32)
                (call (@inst $ByteList.from-array $S) $a $offset $half)
                (call (@inst $ByteList.from-array $S)
                  $a (+ $offset $half) (- $length $half)))))))))

  (func $bytes.len
    (param $b $bytes)
    (result i32)
    (@effect pure)
    (call (@inst $List.len i32) (struct.get $Bytes $data $b)))

  (func $bytes.get
    (param $b $bytes)
    (param $i i32)
    (result i32)
    (@effect pure)
    (call (@inst $List.get i32) (struct.get $Bytes $data $b) $i))

  (func $bytes.slice-tree
    (param $b $bytes)
    (param $offset i32)
    (param $length i32)
    (result (ref (@inst $List i32)))
    (@effect pure)
    (if
      (result (ref (@inst $List i32)))
      (i32.eqz $length)
      (then (call (@inst $List.empty i32)))
      (else
        (if
          (result (ref (@inst $List i32)))
          (= $length 1)
          (then
            (struct.new (@inst $Leaf i32)
              1
              (ref.null (@inst $Marker i32))
              (call $bytes.get $b $offset)))
          (else
            (@let $half (i32.shr_u $length 1)
              (call
                (@inst $List.join i32)
                (call $bytes.slice-tree $b $offset $half)
                (call $bytes.slice-tree $b (+ $offset $half) (- $length $half))))
          ))))
  )

  (func $bytes.slice
    (param $b $bytes)
    (param $offset i32)
    (param $length i32)
    (result $bytes)
    (@effect pure)
    (if
      (result $bytes)
      (||
        (i32.gt_u $offset (call $bytes.len $b))
        (i32.gt_u $length (- (call $bytes.len $b) $offset)))
      (then (unreachable))
      (else (struct.new $Bytes (call $bytes.slice-tree $b $offset $length))))
  )

  (func $bytes.copy-to-array
    (@type.params $S)
    (param $a (ref $S))
    (param $dst i32)
    (param $b $bytes)
    (param $src i32)
    (param $length i32)
    (@effect writes)
    (block
      (if
        (||
          (|| (i32.gt_u $dst (array.len $a)) (i32.gt_u $length (- (array.len $a) $dst)))
          (||
            (i32.gt_u $src (call $bytes.len $b))
            (i32.gt_u $length (- (call $bytes.len $b) $src))))
        (then (unreachable))
        (else (nop)))
      (@for $copy
        (bind $i i32 0)
        (while (i32.lt_u $i $length))
        (step (+ $i 1))
        (array.set $S $a (+ $dst $i) (call $bytes.get $b (+ $src $i))))
    ))

  (func $bytes.is-utf8
    (param $b $bytes)
    (result i32)
    (@effect pure)
    (@loop $scan
      (bind $i i32 0)
      (result i32)
      (if
        (result i32)
        (= $i (call $bytes.len $b))
        (then 1)
        (else
          (@let
            (($c (call $bytes.get $b $i)))
            (if
              (result i32)
              (< $c 128)
              (then (@recur $scan (+ $i 1)))
              (else
                (@let
                  (($n
                     (if
                       (result i32)
                       (&& (>= $c 194) (<= $c 223))
                       (then 1)
                       (else
                         (if
                           (result i32)
                           (&& (>= $c 224) (<= $c 239))
                           (then 2)
                           (else
                             (if
                               (result i32)
                               (&& (>= $c 240) (<= $c 244))
                               (then 3)
                               (else 0))))
                       )))
                   ($lo
                     (if
                       (result i32)
                       (= $c 224)
                       (then 160)
                       (else (if (result i32) (= $c 240) (then 144) (else 128))))
                   )
                   ($hi
                     (if
                       (result i32)
                       (= $c 237)
                       (then 159)
                       (else (if (result i32) (= $c 244) (then 143) (else 191))))
                   ))
                  (if
                    (result i32)
                    (|| (i32.eqz $n) (>= (+ $i $n) (call $bytes.len $b)))
                    (then 0)
                    (else
                      (@let
                        (($first (call $bytes.get $b (+ $i 1)))
                         ($valid
                           (@loop $rest
                             (bind $j i32 2)
                             (result i32)
                             (if
                               (result i32)
                               (> $j $n)
                               (then 1)
                               (else
                                 (@let
                                   (($v (call $bytes.get $b (+ $i $j))))
                                   (if
                                     (result i32)
                                     (&& (>= $v 128) (<= $v 191))
                                     (then (@recur $rest (+ $j 1)))
                                     (else 0))))
                             ))))
                        (if
                          (result i32)
                          (&& $valid (&& (>= $first $lo) (<= $first $hi)))
                          (then (@recur $scan (+ (+ $i $n) 1)))
                          (else 0))))
                  ))))
          ))))
  )

  (func $string.bytes
    (param $s $string)
    (result $bytes)
    (@effect pure)
    (struct.get $String $bytes $s))

  (func $string.from-bytes
    (param $b $bytes)
    (result (@inst $option $string))
    (@effect pure)
    (if
      (result (@inst $option $string))
      (call $bytes.is-utf8 $b)
      (then (@option.some (struct.new $String $b)))
      (else (@option.none))))
  (@export
    (opaque $list-builder)
    (func $list-builder.new)
    (func $list-builder.push)
    (func $list-builder.extend)
    (func $list-builder.len)
    (func $list-builder.finish)
    (func $list-builder.clear))
  (type $Chunk
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (struct (field $value $T) (field $previous (ref null (@inst $Chunk $T))))
  )
  (type $list-builder
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (struct (field $head (mut (ref null (@inst $Chunk $T)))) (field $length (mut i32))))
  (type $Storage (@type.params $T) (@where (shared $T) (nonunit $T)) (array (mut $T)))
  (func $list-builder.new
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (result (ref (@inst $list-builder $T)))
    (@effect allocates)
    (struct.new (@inst $list-builder $T) (ref.null (@inst $Chunk $T)) 0))

  (func $list-builder.len
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $b (ref (@inst $list-builder $T)))
    (result i32)
    (@effect reads)
    (struct.get (@inst $list-builder $T) $length $b))

  (func $list-builder.clear
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $b (ref (@inst $list-builder $T)))
    (@effect writes)
    (block
      (struct.set (@inst $list-builder $T) $head $b (ref.null (@inst $Chunk $T)))
      (struct.set (@inst $list-builder $T) $length $b 0)))

  (func $list-builder.push
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $b (ref (@inst $list-builder $T)))
    (param $value $T)
    (@effect reads writes allocates)
    (@let $length (struct.get (@inst $list-builder $T) $length $b)
      (if
        (= $length -1)
        (then (unreachable))
        (else
          (block
            (struct.set
              (@inst $list-builder $T)
              $head
              $b
              (struct.new (@inst $Chunk $T)
                $value
                (struct.get (@inst $list-builder $T) $head $b)))
            (struct.set (@inst $list-builder $T) $length $b (+ $length 1))))
      )))

  (func $list-builder.extend
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $b (ref (@inst $list-builder $T)))
    (param $values (@inst $list $T))
    (@effect reads writes allocates)
    (@for $append
      (bind $i i32 0)
      (while (i32.lt_u $i (call (@inst $List.len $T) $values)))
      (step (+ $i 1))
      (call (@inst $list-builder.push $T) $b (@list.get $T $values $i))))

  (func $list-builder.copy
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $out (ref (@inst $Storage $T)))
    (param $head (ref (@inst $Chunk $T)))
    (param $length i32)
    (@effect writes)
    (@loop $copy
      (bind $chunk (ref null (@inst $Chunk $T)) $head)
      (bind $end i32 $length)
      (if
        (ref.is_null $chunk)
        (then (nop))
        (else
          (@let $chunk (ref.as_non_null $chunk)
            (block
              (array.set
                (@inst $Storage $T)
                $out
                (- $end 1)
                (struct.get (@inst $Chunk $T) $value $chunk))
              (@recur $copy (struct.get (@inst $Chunk $T) $previous $chunk) (- $end 1))))
        ))))

  (func $list-builder.finish
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $b (ref (@inst $list-builder $T)))
    (result (@inst $list $T))
    (@effect reads writes allocates)
    (@let $length (struct.get (@inst $list-builder $T) $length $b)
      (if
        (result (@inst $list $T))
        (i32.eqz $length)
        (then (call (@inst $List.empty $T)))
        (else
          (@let
            (($head (ref.as_non_null (struct.get (@inst $list-builder $T) $head $b)))
             ($out
               (array.new
                 (@inst $Storage $T)
                 (struct.get (@inst $Chunk $T) $value $head)
                 $length)))
            (block
              (result (@inst $list $T))
              (call (@inst $list-builder.copy $T) $out $head $length)
              (@let $snapshot
                (call (@inst $List.from-array $T (@inst $Storage $T)) $out 0 (array.len $out))
                (block
                  (result (@inst $list $T))
                  (call (@inst $list-builder.clear $T) $b)
                  $snapshot))))
        ))))

  (func $bytes.check
    (param $v i32)
    (result i32)
    (@effect pure)
    (if (result i32) (i32.gt_u $v 255) (then (unreachable)) (else $v)))
  (@export
    (opaque $bytes-builder)
    (func $bytes-builder.new)
    (func $bytes-builder.byte)
    (func $bytes-builder.append)
    (func $bytes-builder.len)
    (func $bytes-builder.finish))
  (type $BytesStorage (array (mut i8)))
  (type $bytes-builder
    (struct
      (field $data (mut (ref $BytesStorage)))
      (field $length (mut i32))
      (field $limit i32)))
  ;; A private GC buffer; finish publishes a copied immutable snapshot.
  (func $bytes-builder.new
    (param $limit i32)
    (result (ref $bytes-builder))
    (@effect allocates)
    (block
      (result (ref $bytes-builder))
      (if (i32.gt_u $limit 16777216) (then (unreachable)) (else (nop)))
      (struct.new $bytes-builder
        (array.new $BytesStorage 0
          (if (result i32) (< $limit 64) (then $limit) (else 64)))
        0
        $limit)))

  (func $bytes-builder.len
    (param $b (ref $bytes-builder))
    (result i32)
    (@effect reads)
    (struct.get $bytes-builder $length $b))

  (func $reserve
    (param $b (ref $bytes-builder))
    (param $extra i32)
    (@effect reads writes allocates)
    (@let
      (($length (call $bytes-builder.len $b))
       ($limit (struct.get $bytes-builder $limit $b)))
      (block
        (if (i32.gt_u $extra (- $limit $length)) (then (unreachable)) (else (nop)))
        (@let
          (($need (+ $length $extra))
           ($old (struct.get $bytes-builder $data $b)))
          (if
            (> $need (array.len $old))
            (then
              (@let $cap
                (@loop $grow
                  (bind $cap i32 (array.len $old))
                  (result i32)
                  (if
                    (result i32)
                    (>= $cap $need)
                    (then $cap)
                    (else
                      (@recur $grow
                        (if
                          (result i32)
                          (>= $cap (i32.shr_u $limit 1))
                          (then $limit)
                          (else (* $cap 2))))
                    )))
                (@let $next (array.new $BytesStorage 0 $cap)
                  (block
                    (array.copy $BytesStorage $BytesStorage $next 0 $old 0 $length)
                    (struct.set $bytes-builder $data $b $next))))
            )
            (else (nop))))
      )))

  (func $bytes-builder.byte
    (param $b (ref $bytes-builder))
    (param $value i32)
    (@effect reads writes allocates)
    (block
      (if (i32.gt_u $value 255) (then (unreachable)) (else (nop)))
      (call $reserve $b 1)
      (@let $n (call $bytes-builder.len $b)
        (block
          (array.set $BytesStorage (struct.get $bytes-builder $data $b) $n $value)
          (struct.set $bytes-builder $length $b (+ $n 1))))
    ))

  (func $bytes-builder.append
    (param $b (ref $bytes-builder))
    (param $value $bytes)
    (@effect reads writes allocates)
    (block
      (call $reserve $b (@bytes.len $value))
      (@let $n (call $bytes-builder.len $b)
        (block
          (@bytes.copy-to-array $BytesStorage
            (struct.get $bytes-builder $data $b)
            $n
            $value
            0
            (@bytes.len $value))
          (struct.set $bytes-builder $length $b (+ $n (@bytes.len $value))))
      )))

  (func $bytes-builder.finish
    (param $b (ref $bytes-builder))
    (result $bytes)
    (@effect reads allocates)
    (@bytes.from-array $BytesStorage
      (struct.get $bytes-builder $data $b)
      0
      (call $bytes-builder.len $b)))
  (@export (func $bytes.equal))
  (func $bytes.equal
    (param $a $bytes)
    (param $b $bytes)
    (result i32)
    (@effect pure)
    (if
      (result i32)
      (!= (call $bytes.len $a) (call $bytes.len $b))
      (then 0)
      (else
        (@loop $equal
          (bind $i i32 0)
          (result i32)
          (if
            (result i32)
            (= $i (call $bytes.len $a))
            (then 1)
            (else
              (if
                (result i32)
                (= (call $bytes.get $a $i) (call $bytes.get $b $i))
                (then (@recur $equal (+ $i 1)))
                (else 0))))
        ))))
)
