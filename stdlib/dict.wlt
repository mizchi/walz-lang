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
      (%effects pure)
      (%let $bytes (%text.bytes (local.get $key))
        (%loop $hash
          (bind $i i32 (i32.const 0))
          (bind $hash i32 (i32.const 2166136261))
          (result i32)
          (if
            (result i32)
            (i32.eq (local.get $i) (%bytes.len (local.get $bytes)))
            (then
              (i32.and (local.get $hash) (i32.sub (local.get $length) (i32.const 1))))
            (else
              (%recur $hash
                (i32.add (local.get $i) (i32.const 1))
                (i32.mul
                  (i32.xor
                    (local.get $hash)
                    (%bytes.get (local.get $bytes) (local.get $i)))
                  (i32.const 16777619))))
          ))))

    (func $new
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (result (ref (%inst $Dict $T)))
      (%effects allocates)
      (struct.new
        (%inst $Dict $T)
        (array.new (%inst $Buckets $T) (ref.null (%inst $Entry $T)) (i32.const 16))
        (i32.const 0)))

    (func $len
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (result i32)
      (%effects reads)
      (struct.get (%inst $Dict $T) $size (local.get $d)))

    (func $find
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (result (ref null (%inst $Entry $T)))
      (%effects reads)
      (%let $buckets (struct.get (%inst $Dict $T) $buckets (local.get $d))
        (%loop $find
          (bind $entry
            (ref null (%inst $Entry $T))
            (array.get
              (%inst $Buckets $T)
              (local.get $buckets)
              (call $slot (local.get $key) (array.len (local.get $buckets))))
          )
          (result (ref null (%inst $Entry $T)))
          (if
            (result (ref null (%inst $Entry $T)))
            (ref.is_null (local.get $entry))
            (then (ref.null (%inst $Entry $T)))
            (else
              (%let $entry (ref.as_non_null (local.get $entry))
                (if
                  (result (ref null (%inst $Entry $T)))
                  (call $buffer.equal
                    (%text.bytes (local.get $key))
                    (%text.bytes (struct.get (%inst $Entry $T) $key (local.get $entry))))
                  (then (local.get $entry))
                  (else
                    (%recur $find
                      (struct.get (%inst $Entry $T) $next (local.get $entry))))
                ))))
        )))

    (func $get
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (result (%inst $option $T))
      (%effects reads)
      (%let $entry (call (%inst $find $T) (local.get $d) (local.get $key))
        (if
          (result (%inst $option $T))
          (ref.is_null (local.get $entry))
          (then (%option.none $T))
          (else
            (%option.some $T
              (struct.get (%inst $Entry $T) $value (ref.as_non_null (local.get $entry))))
          ))))

    (func $grow
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (%effects reads writes allocates)
      (%let
        (($old (struct.get (%inst $Dict $T) $buckets (local.get $d)))
         ($length (array.len (local.get $old))))
        (if
          (i32.ge_u (local.get $length) (i32.const 1073741824))
          (then (unreachable))
          (else
            (%let $new
              (array.new (%inst $Buckets $T) (ref.null (%inst $Entry $T)) (i32.mul (local.get $length) (i32.const 2)))
              (block
                (%for $buckets
                  (bind $i i32 (i32.const 0))
                  (while (i32.lt_u (local.get $i) (local.get $length)))
                  (step (i32.add (local.get $i) (i32.const 1)))
                  (%loop $chain
                    (bind $entry
                      (ref null (%inst $Entry $T))
                      (array.get (%inst $Buckets $T) (local.get $old) (local.get $i)))
                    (if
                      (ref.is_null (local.get $entry))
                      (then (nop))
                      (else
                        (%let
                          (($entry (ref.as_non_null (local.get $entry)))
                           ($next
                             (struct.get (%inst $Entry $T) $next (local.get $entry)))
                           ($slot
                             (call $slot
                               (struct.get (%inst $Entry $T) $key (local.get $entry))
                               (array.len (local.get $new))))
                          )
                          (block
                            (struct.set
                              (%inst $Entry $T)
                              $next
                              (local.get $entry)
                              (array.get
                                (%inst $Buckets $T)
                                (local.get $new)
                                (local.get $slot)))
                            (array.set
                              (%inst $Buckets $T)
                              (local.get $new)
                              (local.get $slot)
                              (local.get $entry))
                            (%recur $chain (local.get $next))))
                      ))))
                (struct.set (%inst $Dict $T) $buckets (local.get $d) (local.get $new))))
          ))))

    (func $set
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (param $value $T)
      (result (%inst $option $T))
      (%effects reads writes allocates)
      (%let $entry (call (%inst $find $T) (local.get $d) (local.get $key))
        (if
          (result (%inst $option $T))
          (ref.is_null (local.get $entry))
          (then
            (block
              (result (%inst $option $T))
              (if
                (i32.ge_u
                  (struct.get (%inst $Dict $T) $size (local.get $d))
                  (array.len (struct.get (%inst $Dict $T) $buckets (local.get $d))))
                (then (call (%inst $grow $T) (local.get $d)))
                (else (nop)))
              (%let
                (($buckets (struct.get (%inst $Dict $T) $buckets (local.get $d)))
                 ($slot (call $slot (local.get $key) (array.len (local.get $buckets))))
                )
                (block
                  (result (%inst $option $T))
                  (array.set
                    (%inst $Buckets $T)
                    (local.get $buckets)
                    (local.get $slot)
                    (struct.new
                      (%inst $Entry $T)
                      (local.get $key)
                      (local.get $value)
                      (array.get
                        (%inst $Buckets $T)
                        (local.get $buckets)
                        (local.get $slot))))
                  (struct.set
                    (%inst $Dict $T)
                    $size
                    (local.get $d)
                    (i32.add
                      (struct.get (%inst $Dict $T) $size (local.get $d))
                      (i32.const 1)))
                  (%option.none $T))))
          )
          (else
            (%let
              (($entry (ref.as_non_null (local.get $entry)))
               ($previous (struct.get (%inst $Entry $T) $value (local.get $entry))))
              (block
                (result (%inst $option $T))
                (struct.set
                  (%inst $Entry $T)
                  $value
                  (local.get $entry)
                  (local.get $value))
                (%option.some $T (local.get $previous))))
          ))))

    (func $remove
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (param $key $text)
      (result (%inst $option $T))
      (%effects reads writes)
      (%let
        (($buckets (struct.get (%inst $Dict $T) $buckets (local.get $d)))
         ($slot (call $slot (local.get $key) (array.len (local.get $buckets))))
        )
        (%loop $remove
          (bind $entry
            (ref null (%inst $Entry $T))
            (array.get (%inst $Buckets $T) (local.get $buckets) (local.get $slot)))
          (bind $previous (ref null (%inst $Entry $T)) (ref.null (%inst $Entry $T)))
          (result (%inst $option $T))
          (if
            (result (%inst $option $T))
            (ref.is_null (local.get $entry))
            (then (%option.none $T))
            (else
              (%let $entry (ref.as_non_null (local.get $entry))
                (if
                  (result (%inst $option $T))
                  (call $buffer.equal
                    (%text.bytes (local.get $key))
                    (%text.bytes (struct.get (%inst $Entry $T) $key (local.get $entry))))
                  (then
                    (%let $next (struct.get (%inst $Entry $T) $next (local.get $entry))
                      (block
                        (result (%inst $option $T))
                        (if
                          (ref.is_null (local.get $previous))
                          (then
                            (array.set
                              (%inst $Buckets $T)
                              (local.get $buckets)
                              (local.get $slot)
                              (local.get $next)))
                          (else
                            (struct.set
                              (%inst $Entry $T)
                              $next
                              (ref.as_non_null (local.get $previous))
                              (local.get $next))))
                        (struct.set
                          (%inst $Entry $T)
                          $next
                          (local.get $entry)
                          (ref.null (%inst $Entry $T)))
                        (struct.set
                          (%inst $Dict $T)
                          $size
                          (local.get $d)
                          (i32.sub
                            (struct.get (%inst $Dict $T) $size (local.get $d))
                            (i32.const 1)))
                        (%option.some $T
                          (struct.get (%inst $Entry $T) $value (local.get $entry))))
                    ))
                  (else
                    (%recur $remove
                      (struct.get (%inst $Entry $T) $next (local.get $entry))
                      (local.get $entry))))
              ))))
      ))

    (func $clear
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (%effects reads writes)
      (%let $buckets (struct.get (%inst $Dict $T) $buckets (local.get $d))
        (block
          (%for $clear
            (bind $i i32 (i32.const 0))
            (while (i32.lt_u (local.get $i) (array.len (local.get $buckets))))
            (step (i32.add (local.get $i) (i32.const 1)))
            (array.set
              (%inst $Buckets $T)
              (local.get $buckets)
              (local.get $i)
              (ref.null (%inst $Entry $T))))
          (struct.set (%inst $Dict $T) $size (local.get $d) (i32.const 0))))
    )

    (func $entries
      (%type.params $T)
      (%where (shared $T) (nonunit $T))
      (param $d (ref (%inst $Dict $T)))
      (result (%inst $list (%inst $pair $text $T)))
      (%effects reads writes allocates)
      (%let
        (($buckets (struct.get (%inst $Dict $T) $buckets (local.get $d)))
         ($out (call (%inst $list-builder.new (%inst $pair $text $T))))
        )
        (block
          (result (%inst $list (%inst $pair $text $T)))
          (%for $buckets
            (bind $i i32 (i32.const 0))
            (while (i32.lt_u (local.get $i) (array.len (local.get $buckets))))
            (step (i32.add (local.get $i) (i32.const 1)))
            (%loop $chain
              (bind $entry
                (ref null (%inst $Entry $T))
                (array.get (%inst $Buckets $T) (local.get $buckets) (local.get $i)))
              (if
                (ref.is_null (local.get $entry))
                (then (nop))
                (else
                  (%let $entry (ref.as_non_null (local.get $entry))
                    (block
                      (call
                        (%inst $list-builder.push (%inst $pair $text $T))
                        (local.get $out)
                        (%pair.new
                          (struct.get (%inst $Entry $T) $key (local.get $entry))
                          (struct.get (%inst $Entry $T) $value (local.get $entry))))
                      (%recur $chain
                        (struct.get (%inst $Entry $T) $next (local.get $entry))))
                  ))))
          )
          (call (%inst $list-builder.finish (%inst $pair $text $T)) (local.get $out))))
    )))
