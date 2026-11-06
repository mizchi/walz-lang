(module
  (%namespace $builder
    (%import "wlt:std" (type $text))
    (%import "text.wlt" (func $string.json) (func $string.s64) (func $string.u64))
    (%export
      (opaque $Builder)
      (func $new)
      (func $text)
      (func $json)
      (func $u64)
      (func $s64)
      (func $finish))
    (type $Chunk (struct (field $text $text) (field $previous (ref null $Chunk))))
    (type $Builder
      (struct (field $head (mut (ref null $Chunk))) (field $length (mut i32))))
    (type $Array (array (mut i32)))
    (func $new
      (result (ref $Builder))
      (%effect allocates)
      (struct.new $Builder (ref.null $Chunk) 0))

    (func $text
      (param $b (ref $Builder))
      (param $s $text)
      (%effect reads writes allocates)
      (%let $n (%bytes.len (%text.bytes $s))
        (if $n
          (then
            (%let $previous (struct.get $Builder $length $b)
              (if
                (i32.gt_u $n (- -1 $previous))
                (then (unreachable))
                (else
                  (block
                    (struct.set $Builder $head $b
                      (struct.new $Chunk $s (struct.get $Builder $head $b)))
                    (struct.set $Builder $length $b (+ $previous $n))))
              )))
          (else (block))))
    )

    (func $json
      (param $b (ref $Builder))
      (param $s $text)
      (%effect reads writes allocates)
      (call $text $b (call $string.json $s)))

    (func $u64
      (param $b (ref $Builder))
      (param $n i64)
      (%effect reads writes allocates)
      (call $text $b (call $string.u64 $n)))

    (func $s64
      (param $b (ref $Builder))
      (param $n i64)
      (%effect reads writes allocates)
      (call $text $b (call $string.s64 $n)))
    ;; Traverse backwards, placing each fragment at its final offset. No recursion.
    (func $finish
      (param $b (ref $Builder))
      (result $text)
      (%effect reads writes allocates)
      (%let
        (($length (struct.get $Builder $length $b))
         ($out (array.new $Array 0 $length)))
        (block
          (result $text)
          (%loop $chunks
            (bind $chunk (ref null $Chunk) (struct.get $Builder $head $b))
            (bind $end i32 $length)
            (if
              (ref.is_null $chunk)
              (then (block))
              (else
                (%let
                  (($chunk (ref.as_non_null $chunk))
                   ($bytes (%text.bytes (struct.get $Chunk $text $chunk)))
                   ($start (- $end (%bytes.len $bytes))))
                  (block
                    (%for $copy
                      (bind $i i32 0)
                      (while (i32.lt_u $i (%bytes.len $bytes)))
                      (step (+ $i 1))
                      (array.set $Array $out (+ $start $i) (%bytes.get $bytes $i)))
                    (%recur $chunks (struct.get $Chunk $previous $chunk) $start))))
            ))
          (struct.set $Builder $head $b (ref.null $Chunk))
          (struct.set $Builder $length $b 0)
          (%match (%text.from-bytes (%bytes.from-array $Array $out))
            (case none () (unreachable))
            (case some ($s) $s))))
    )))
