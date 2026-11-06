(module
  (%namespace $stock
    (%import "wlt:std"
      (type $bytes)
      (type $result)
      (type $text))
    (%public
      (opaque $State)
      (type $Error)
      (func $new)
      (func $size)
      (func $get)
      (func $add)
      (func $take)
    )
    (type $Error (%variant (insufficient) (overflow) (capacity)))
    (type $Entry
      (struct
        (field $key $bytes)
        (field $quantity (mut i64))
        (field $next (ref null $Entry))
      )
    )
    (type $Buckets (array (mut (ref null $Entry))))
    (type $State
      (struct
        (field $buckets (ref $Buckets))
        (field $size (mut i32))
        (field $limit i32)
      )
    )
    (func $new
      (param $limit i32)
      (result (ref $State))
      (%effect allocates)
      (struct.new $State
        (array.new $Buckets (ref.null $Entry) (i32.const 64))
        (i32.const 0)
        (local.get $limit)
      )
    )
    (func $size
      (param $s (ref $State))
      (result i32)
      (%effect reads)
      (struct.get $State $size (local.get $s))
    )
    (func $hash
      (param $key $bytes)
      (result i32)
      (%effect pure)
      (%loop $hash
        (bind $i i32 (i32.const 0))
        (bind $h i32 (i32.const 0))
        (result i32)
        (if
          (result i32)
          (i32.eq (local.get $i) (%bytes.len (local.get $key)))
          (then (i32.and (local.get $h) (i32.const 63)))
          (else
            (%recur $hash
              (i32.add (local.get $i) (i32.const 1))
              (i32.add
                (i32.mul (local.get $h) (i32.const 31))
                (%bytes.get (local.get $key) (local.get $i))
              )
            )
          )
        )
      )
    )
    (func $same
      (param $a $bytes)
      (param $b $bytes)
      (result i32)
      (%effect pure)
      (if
        (result i32)
        (i32.ne (%bytes.len (local.get $a)) (%bytes.len (local.get $b)))
        (then (i32.const 0))
        (else
          (%loop $same
            (bind $i i32 (i32.const 0))
            (result i32)
            (if
              (result i32)
              (i32.eq (local.get $i) (%bytes.len (local.get $a)))
              (then (i32.const 1))
              (else
                (if
                  (result i32)
                  (i32.ne
                    (%bytes.get (local.get $a) (local.get $i))
                    (%bytes.get (local.get $b) (local.get $i))
                  )
                  (then (i32.const 0))
                  (else (%recur $same (i32.add (local.get $i) (i32.const 1))))
                )
              )
            )
          )
        )
      )
    )
    (func $find
      (param $s (ref $State))
      (param $key $bytes)
      (result (ref null $Entry))
      (%effect reads)
      (%loop $find
        (bind $entry
          (ref null $Entry)
          (array.get $Buckets
            (struct.get $State $buckets (local.get $s))
            (call $hash (local.get $key))
          )
        )
        (result (ref null $Entry))
        (if
          (result (ref null $Entry))
          (ref.is_null (local.get $entry))
          (then (local.get $entry))
          (else
            (%let $node
              (ref.as_non_null (local.get $entry))
              (if
                (result (ref null $Entry))
                (call $same (struct.get $Entry $key (local.get $node)) (local.get $key))
                (then (local.get $node))
                (else (%recur $find (struct.get $Entry $next (local.get $node))))
              )
            )
          )
        )
      )
    )
    (func $get
      (param $s (ref $State))
      (param $key $text)
      (result i64)
      (%effect reads)
      (%let $found
        (call $find (local.get $s) (%text.bytes (local.get $key)))
        (if
          (result i64)
          (ref.is_null (local.get $found))
          (then (i64.const 0))
          (else (struct.get $Entry $quantity (ref.as_non_null (local.get $found))))
        )
      )
    )
    (func $add
      (param $s (ref $State))
      (param $key $text)
      (param $n i64)
      (result (%inst $result i64 (ref $Error)))
      (%effect reads writes allocates)
      (%let $bytes
        (%text.bytes (local.get $key))
        (%let $found
          (call $find (local.get $s) (local.get $bytes))
          (if
            (result (%inst $result i64 (ref $Error)))
            (ref.is_null (local.get $found))
            (then
              (if
                (result (%inst $result i64 (ref $Error)))
                (i32.ge_u
                  (call $size (local.get $s))
                  (struct.get $State $limit (local.get $s))
                )
                (then (%result.err i64 (ref $Error) (%variant.new $Error capacity)))
                (else
                  (%let $buckets
                    (struct.get $State $buckets (local.get $s))
                    (%let $index
                      (call $hash (local.get $bytes))
                      (block
                        (result (%inst $result i64 (ref $Error)))
                        (array.set $Buckets
                          (local.get $buckets)
                          (local.get $index)
                          (struct.new $Entry
                            (local.get $bytes)
                            (local.get $n)
                            (array.get $Buckets (local.get $buckets) (local.get $index))
                          )
                        )
                        (struct.set $State $size
                          (local.get $s)
                          (i32.add (call $size (local.get $s)) (i32.const 1))
                        )
                        (%result.ok i64 (ref $Error) (local.get $n))
                      )
                    )
                  )
                )
              )
            )
            (else
              (%let $node
                (ref.as_non_null (local.get $found))
                (%let $before
                  (struct.get $Entry $quantity (local.get $node))
                  (%let $sum
                    (i64.add (local.get $before) (local.get $n))
                    (if
                      (result (%inst $result i64 (ref $Error)))
                      (i64.lt_u (local.get $sum) (local.get $before))
                      (then (%result.err i64 (ref $Error) (%variant.new $Error overflow)))
                      (else
                        (block
                          (result (%inst $result i64 (ref $Error)))
                          (struct.set $Entry $quantity
                            (local.get $node)
                            (local.get $sum)
                          )
                          (%result.ok i64 (ref $Error) (local.get $sum))
                        )
                      )
                    )
                  )
                )
              )
            )
          )
        )
      )
    )
    (func $take
      (param $s (ref $State))
      (param $key $text)
      (param $n i64)
      (result (%inst $result i64 (ref $Error)))
      (%effect reads writes allocates)
      (%let $found
        (call $find (local.get $s) (%text.bytes (local.get $key)))
        (%let $before
          (if
            (result i64)
            (ref.is_null (local.get $found))
            (then (i64.const 0))
            (else (struct.get $Entry $quantity (ref.as_non_null (local.get $found))))
          )
          (if
            (result (%inst $result i64 (ref $Error)))
            (i64.gt_u (local.get $n) (local.get $before))
            (then (%result.err i64 (ref $Error) (%variant.new $Error insufficient)))
            (else
              (%let $after
                (i64.sub (local.get $before) (local.get $n))
                (block
                  (result (%inst $result i64 (ref $Error)))
                  (if
                    (ref.is_null (local.get $found))
                    (then (nop))
                    (else
                      (struct.set $Entry $quantity
                        (ref.as_non_null (local.get $found))
                        (local.get $after)
                      )
                    )
                  )
                  (%result.ok i64 (ref $Error) (local.get $after))
                )
              )
            )
          )
        )
      )
    )
  )
)
