(module
  (namespace $builder
    (import "text.wlt" (func $string.json) (func $string.s64) (func $string.u64))
    (export
      (opaque $Builder)
      (func $new)
      (func $text)
      (func $json)
      (func $u64)
      (func $s64)
      (func $finish))
    (type $Chunk (struct (field $text text) (field $previous (ref null $Chunk))))
    (type $Builder
      (struct (field $head (mut (ref null $Chunk))) (field $length (mut i32))))
    (type $Array (array (mut i32)))
    (func $new (result (ref $Builder)) (effects allocates)
      (struct.new $Builder (ref.null $Chunk) (i32.const 0)))

    (func $text (param $b (ref $Builder)) (param $s text)
      (effects reads writes allocates)
      (let $n (bytes.len (text.bytes (local.get $s)))
        (if
          (local.get $n)
          (then
            (let $previous (struct.get $Builder $length (local.get $b))
              (if
                (i32.gt_u (local.get $n) (i32.sub (i32.const -1) (local.get $previous)))
                (then (unreachable))
                (else
                  (block
                    (struct.set $Builder $head
                      (local.get $b)
                      (struct.new $Chunk
                        (local.get $s)
                        (struct.get $Builder $head (local.get $b))))
                    (struct.set $Builder $length
                      (local.get $b)
                      (i32.add (local.get $previous) (local.get $n))))
                ))))
          (else (block))))
    )

    (func $json (param $b (ref $Builder)) (param $s text)
      (effects reads writes allocates)
      (call $text (local.get $b) (call $string.json (local.get $s))))

    (func $u64 (param $b (ref $Builder)) (param $n i64) (effects reads writes allocates)
      (call $text (local.get $b) (call $string.u64 (local.get $n))))

    (func $s64 (param $b (ref $Builder)) (param $n i64) (effects reads writes allocates)
      (call $text (local.get $b) (call $string.s64 (local.get $n))))
    ;; Traverse backwards, placing each fragment at its final offset. No recursion.
    (func $finish (param $b (ref $Builder)) (result text)
      (effects reads writes allocates)
      (let $length (struct.get $Builder $length (local.get $b))
        (let $out (array.new $Array (i32.const 0) (local.get $length))
          (block
            (result text)
            (loop $chunks
              (bind $chunk (ref null $Chunk) (struct.get $Builder $head (local.get $b)))
              (bind $end i32 (local.get $length))
              (if
                (ref.is_null (local.get $chunk))
                (then (block))
                (else
                  (let $chunk (ref.as_non_null (local.get $chunk))
                    (let $bytes
                      (text.bytes (struct.get $Chunk $text (local.get $chunk)))
                      (let $start
                        (i32.sub (local.get $end) (bytes.len (local.get $bytes)))
                        (block
                          (for $copy
                            (bind $i i32 (i32.const 0))
                            (while
                              (i32.lt_u (local.get $i) (bytes.len (local.get $bytes))))
                            (step (i32.add (local.get $i) (i32.const 1)))
                            (array.set $Array
                              (local.get $out)
                              (i32.add (local.get $start) (local.get $i))
                              (bytes.get (local.get $bytes) (local.get $i))))
                          (recur $chunks
                            (struct.get $Chunk $previous (local.get $chunk))
                            (local.get $start))))
                    ))))
            )
            (struct.set $Builder $head (local.get $b) (ref.null $Chunk))
            (struct.set $Builder $length (local.get $b) (i32.const 0))
            (match (text.from-bytes (bytes.from-array $Array (local.get $out)))
              (case none () (unreachable))
              (case some ($s) (local.get $s))))
        ))))
)
