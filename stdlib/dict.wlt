(module
  (%namespace $dict
    (%import "wlt:std" (type $text) (type $option) (type $list) (type $pair))
    (%import "list-builder.wlt"
      (func $list-builder.new)
      (func $list-builder.push)
      (func $list-builder.finish))
    (%import "bytes.wlt" (func $buffer.equal))
    (%export
      (opaque $Dict)
      (func $new)
      (func $len)
      (func $get)
      (func $set)
      (func $remove)
      (func $clear)
      (func $entries))
    (type $Entry
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (struct
        (field $key $text)
        (field $value (mut $T))
        (field $next (mut (ref null (%inst $Entry $T))))
      ))
    (type $Buckets
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (array (mut (ref null (%inst $Entry $T))))
    )
    (type $Dict
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (struct (field $buckets (mut (ref (%inst $Buckets $T)))) (field $size (mut i32))))
    (func $slot
      (param $key $text)
      (param $length i32)
      (result i32)
      (%effect pure)
      (%let $bytes (%text.bytes $key)
        (%loop $hash
          (bind $i i32 0)
          (bind $hash i32 2166136261)
          (result i32)
          (if
            (result i32)
            (= $i (%bytes.len $bytes))
            (then (& $hash (- $length 1)))
            (else (%recur $hash (+ $i 1) (* (^ $hash (%bytes.get $bytes $i)) 16777619))))
        )))

    (func $new
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (result (ref (%inst $Dict $T)))
      (%effect allocates)
      (struct.new (%inst $Dict $T)
        (array.new (%inst $Buckets $T) (ref.null (%inst $Entry $T)) 16)
        0))

    (func $len
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (result i32)
      (%effect reads)
      (struct.get (%inst $Dict $T) $size $d))

    (func $find
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (result (ref null (%inst $Entry $T)))
      (%effect reads)
      (%let $buckets (struct.get (%inst $Dict $T) $buckets $d)
        (%loop $find
          (bind $entry
            (ref null (%inst $Entry $T))
            (array.get
              (%inst $Buckets $T)
              $buckets
              (call $slot $key (array.len $buckets))))
          (result (ref null (%inst $Entry $T)))
          (if
            (result (ref null (%inst $Entry $T)))
            (ref.is_null $entry)
            (then (ref.null (%inst $Entry $T)))
            (else
              (%let $entry (ref.as_non_null $entry)
                (if
                  (result (ref null (%inst $Entry $T)))
                  (call $buffer.equal
                    (%text.bytes $key)
                    (%text.bytes (struct.get (%inst $Entry $T) $key $entry)))
                  (then $entry)
                  (else (%recur $find (struct.get (%inst $Entry $T) $next $entry))))
              ))))
      ))

    (func $get
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (result (%inst $option $T))
      (%effect reads)
      (%let $entry (call (%inst $find $T) $d $key)
        (if
          (result (%inst $option $T))
          (ref.is_null $entry)
          (then (%option.none))
          (else
            (%option.some
              (struct.get (%inst $Entry $T) $value (ref.as_non_null $entry))))
        )))

    (func $grow
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (%effect reads writes allocates)
      (%let
        (($old (struct.get (%inst $Dict $T) $buckets $d))
         ($length (array.len $old)))
        (if
          (i32.ge_u $length 1073741824)
          (then (unreachable))
          (else
            (%let $new
              (array.new (%inst $Buckets $T) (ref.null (%inst $Entry $T)) (* $length 2))
              (block
                (%for $buckets
                  (bind $i i32 0)
                  (while (i32.lt_u $i $length))
                  (step (+ $i 1))
                  (%loop $chain
                    (bind $entry
                      (ref null (%inst $Entry $T))
                      (array.get (%inst $Buckets $T) $old $i))
                    (if
                      (ref.is_null $entry)
                      (then (nop))
                      (else
                        (%let
                          (($entry (ref.as_non_null $entry))
                           ($next (struct.get (%inst $Entry $T) $next $entry))
                           ($slot
                             (call $slot
                               (struct.get (%inst $Entry $T) $key $entry)
                               (array.len $new))))
                          (block
                            (struct.set
                              (%inst $Entry $T)
                              $next
                              $entry
                              (array.get (%inst $Buckets $T) $new $slot))
                            (array.set (%inst $Buckets $T) $new $slot $entry)
                            (%recur $chain $next))))
                    )))
                (struct.set (%inst $Dict $T) $buckets $d $new))))
        )))

    (func $set
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (param $value $T)
      (result (%inst $option $T))
      (%effect reads writes allocates)
      (%let $entry (call (%inst $find $T) $d $key)
        (if
          (result (%inst $option $T))
          (ref.is_null $entry)
          (then
            (block
              (result (%inst $option $T))
              (if
                (i32.ge_u
                  (struct.get (%inst $Dict $T) $size $d)
                  (array.len (struct.get (%inst $Dict $T) $buckets $d)))
                (then (call (%inst $grow $T) $d))
                (else (nop)))
              (%let
                (($buckets (struct.get (%inst $Dict $T) $buckets $d))
                 ($slot (call $slot $key (array.len $buckets))))
                (block
                  (result (%inst $option $T))
                  (array.set
                    (%inst $Buckets $T)
                    $buckets
                    $slot
                    (struct.new (%inst $Entry $T)
                      $key
                      $value
                      (array.get (%inst $Buckets $T) $buckets $slot)))
                  (struct.set
                    (%inst $Dict $T)
                    $size
                    $d
                    (+ (struct.get (%inst $Dict $T) $size $d) 1))
                  (%option.none))))
          )
          (else
            (%let
              (($entry (ref.as_non_null $entry))
               ($previous (struct.get (%inst $Entry $T) $value $entry)))
              (block
                (result (%inst $option $T))
                (struct.set (%inst $Entry $T) $value $entry $value)
                (%option.some $previous))))
        )))

    (func $remove
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (result (%inst $option $T))
      (%effect reads writes)
      (%let
        (($buckets (struct.get (%inst $Dict $T) $buckets $d))
         ($slot (call $slot $key (array.len $buckets))))
        (%loop $remove
          (bind $entry
            (ref null (%inst $Entry $T))
            (array.get (%inst $Buckets $T) $buckets $slot))
          (bind $previous (ref null (%inst $Entry $T)) (ref.null (%inst $Entry $T)))
          (result (%inst $option $T))
          (if
            (result (%inst $option $T))
            (ref.is_null $entry)
            (then (%option.none))
            (else
              (%let $entry (ref.as_non_null $entry)
                (if
                  (result (%inst $option $T))
                  (call $buffer.equal
                    (%text.bytes $key)
                    (%text.bytes (struct.get (%inst $Entry $T) $key $entry)))
                  (then
                    (%let $next (struct.get (%inst $Entry $T) $next $entry)
                      (block
                        (result (%inst $option $T))
                        (if
                          (ref.is_null $previous)
                          (then (array.set (%inst $Buckets $T) $buckets $slot $next))
                          (else
                            (struct.set
                              (%inst $Entry $T)
                              $next
                              (ref.as_non_null $previous)
                              $next)))
                        (struct.set
                          (%inst $Entry $T)
                          $next
                          $entry
                          (ref.null (%inst $Entry $T)))
                        (struct.set
                          (%inst $Dict $T)
                          $size
                          $d
                          (- (struct.get (%inst $Dict $T) $size $d) 1))
                        (%option.some (struct.get (%inst $Entry $T) $value $entry))))
                  )
                  (else
                    (%recur $remove (struct.get (%inst $Entry $T) $next $entry) $entry))))
            ))))
    )

    (func $clear
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (%effect reads writes)
      (%let $buckets (struct.get (%inst $Dict $T) $buckets $d)
        (block
          (%for $clear
            (bind $i i32 0)
            (while (i32.lt_u $i (array.len $buckets)))
            (step (+ $i 1))
            (array.set (%inst $Buckets $T) $buckets $i (ref.null (%inst $Entry $T))))
          (struct.set (%inst $Dict $T) $size $d 0))))

    (func $entries
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (result (%inst $list (%inst $pair $text $T)))
      (%effect reads writes allocates)
      (%let
        (($buckets (struct.get (%inst $Dict $T) $buckets $d))
         ($out (call (%inst $list-builder.new (%inst $pair $text $T))))
        )
        (block
          (result (%inst $list (%inst $pair $text $T)))
          (%for $buckets
            (bind $i i32 0)
            (while (i32.lt_u $i (array.len $buckets)))
            (step (+ $i 1))
            (%loop $chain
              (bind $entry
                (ref null (%inst $Entry $T))
                (array.get (%inst $Buckets $T) $buckets $i))
              (if
                (ref.is_null $entry)
                (then (nop))
                (else
                  (%let $entry (ref.as_non_null $entry)
                    (block
                      (call
                        (%inst $list-builder.push (%inst $pair $text $T))
                        $out
                        (%pair.new
                          (struct.get (%inst $Entry $T) $key $entry)
                          (struct.get (%inst $Entry $T) $value $entry)))
                      (%recur $chain (struct.get (%inst $Entry $T) $next $entry))))
                ))))
          (call (%inst $list-builder.finish (%inst $pair $text $T)) $out))))
  ))
