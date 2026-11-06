(module
  (%import "wlt:std" (type $result))
  

  (%import "wlt:std"
    (type $result)
    (type $stream)
    (type $string))
  

  (%namespace $parse
    (%import "wlt:std"
      (type $bytes)
      (type $option)
      (type $string))
    
(%use $wlt-file-0 (func $bytes.find-byte)
      (func $bytes.parse-u64))

    (%public (type $Command) (func $command))
    (type $Command
      (%variant (skip) (invalid) (add $string i64) (take $string i64) (show $string)))
    (func $tab (param $b $bytes) (param $start i32) (result i32) (%effect pure)
      (call $bytes.find-byte (local.get $b) (local.get $start) (i32.const 9)))

    (func $op (param $b $bytes) (param $end i32) (result i32) (%effect pure)
      (if
        (result i32)
        (i32.eq (local.get $end) (i32.const 3))
        (then
          (if
            (result i32)
            (i32.and
              (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 65))
              (i32.and
                (i32.eq (%bytes.get (local.get $b) (i32.const 1)) (i32.const 68))
                (i32.eq (%bytes.get (local.get $b) (i32.const 2)) (i32.const 68))))
            (then (i32.const 1))
            (else (i32.const 0))))
        (else
          (if
            (result i32)
            (i32.eq (local.get $end) (i32.const 4))
            (then
              (if
                (result i32)
                (i32.and
                  (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 84))
                  (i32.and
                    (i32.eq (%bytes.get (local.get $b) (i32.const 1)) (i32.const 65))
                    (i32.and
                      (i32.eq (%bytes.get (local.get $b) (i32.const 2)) (i32.const 75))
                      (i32.eq (%bytes.get (local.get $b) (i32.const 3)) (i32.const 69))))
                )
                (then (i32.const 2))
                (else
                  (if
                    (result i32)
                    (i32.and
                      (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 83))
                      (i32.and
                        (i32.eq (%bytes.get (local.get $b) (i32.const 1)) (i32.const 72))
                        (i32.and
                          (i32.eq
                            (%bytes.get (local.get $b) (i32.const 2))
                            (i32.const 79))
                          (i32.eq
                            (%bytes.get (local.get $b) (i32.const 3))
                            (i32.const 87))))
                    )
                    (then (i32.const 3))
                    (else (i32.const 0))))
              ))
            (else (i32.const 0))))
      ))

    (func $number (param $b $bytes) (param $start i32) (result (%inst $option i64))
      (%effect pure)
      (call $bytes.parse-u64
        (local.get $b)
        (local.get $start)
        (i32.sub (%bytes.len (local.get $b)) (local.get $start))
        (i64.const 4294967295)))

    (func $key (param $b $bytes) (param $start i32) (param $end i32)
      (result (%inst $option $string)) (%effect pure)
      (%let $len (i32.sub (local.get $end) (local.get $start))
        (if
          (result (%inst $option $string))
          (i32.or
            (i32.eqz (local.get $len))
            (i32.gt_u (local.get $len) (i32.const 256)))
          (then (%option.none $string))
          (else
            (%string.from-bytes
              (%bytes.slice (local.get $b) (local.get $start) (local.get $len))))
        )))

    (func $fields (param $b $bytes) (result (ref $Command)) (%effect allocates)
      (%let $first (call $tab (local.get $b) (i32.const 0))
        (%let $op (call $op (local.get $b) (local.get $first))
          (if
            (result (ref $Command))
            (i32.eqz (local.get $op))
            (then (%variant.new $Command invalid))
            (else
              (%let $end
                (call $tab (local.get $b) (i32.add (local.get $first) (i32.const 1)))
                (if
                  (result (ref $Command))
                  (if
                    (result i32)
                    (i32.eq (local.get $op) (i32.const 3))
                    (then (i32.ne (local.get $end) (%bytes.len (local.get $b))))
                    (else
                      (i32.or
                        (i32.eq (local.get $end) (%bytes.len (local.get $b)))
                        (i32.ne
                          (call $tab
                            (local.get $b)
                            (i32.add (local.get $end) (i32.const 1)))
                          (%bytes.len (local.get $b))))
                    ))
                  (then (%variant.new $Command invalid))
                  (else
                    (%match (%inst $option $string) (result (ref $Command))
                      (call $key
                        (local.get $b)
                        (i32.add (local.get $first) (i32.const 1))
                        (local.get $end))
                      (case none () (%variant.new $Command invalid))
                      (case some ($key)
                        (if
                          (result (ref $Command))
                          (i32.eq (local.get $op) (i32.const 3))
                          (then (%variant.new $Command show (local.get $key)))
                          (else
                            (%match (%inst $option i64) (result (ref $Command))
                              (call $number
                                (local.get $b)
                                (i32.add (local.get $end) (i32.const 1)))
                              (case none () (%variant.new $Command invalid))
                              (case some ($n)
                                (if
                                  (result (ref $Command))
                                  (i32.eq (local.get $op) (i32.const 1))
                                  (then
                                    (%variant.new $Command add
                                      (local.get $key)
                                      (local.get $n)))
                                  (else
                                    (%variant.new $Command take
                                      (local.get $key)
                                      (local.get $n))))
                              ))))
                      ))))
              ))))
      ))

    (func $command (param $line $string) (result (ref $Command)) (%effect allocates)
      (%let $b (%string.bytes (local.get $line))
        (if
          (result (ref $Command))
          (i32.eqz (%bytes.len (local.get $b)))
          (then (%variant.new $Command skip))
          (else
            (if
              (result (ref $Command))
              (i32.eq (%bytes.get (local.get $b) (i32.const 0)) (i32.const 35))
              (then (%variant.new $Command skip))
              (else (call $fields (local.get $b))))
          ))))
  )

  

  (%namespace $stock
    (%import "wlt:std"
      (type $bytes)
      (type $result)
      (type $string))
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
      (param $key $string)
      (result i64)
      (%effect reads)
      (%let $found
        (call $find (local.get $s) (%string.bytes (local.get $key)))
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
      (param $key $string)
      (param $n i64)
      (result (%inst $result i64 (ref $Error)))
      (%effect reads writes allocates)
      (%let $bytes
        (%string.bytes (local.get $key))
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
      (param $key $string)
      (param $n i64)
      (result (%inst $result i64 (ref $Error)))
      (%effect reads writes allocates)
      (%let $found
        (call $find (local.get $s) (%string.bytes (local.get $key)))
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


  

  (%namespace $report
    (%import "wlt:std"
      (type $future)
      (type $result)
      (type $string)
      (type $unit))
    
(%use $wlt-file-5 (type $json-object)
      (func $json.object-builder.boolean)
      (func $json.object-builder.finish)
      (func $json.object-builder.new)
      (func $json.object-builder.string)
      (func $json.object-builder.u64))

    (import "wlt:host/text-io@0.1.0" "write-line"
      (func $write (param $line $string) (result (%inst $future (%inst $result $unit i32))))
    )
    (%public (func $success) (func $failure) (func $invalid))
    (func $error-name (param $e (ref $stock.Error)) (result $string) (%effect pure)
      (%match $stock.Error (result $string)
        (local.get $e)
        (case insufficient () (%string.lit "insufficient-stock"))
        (case overflow () (%string.lit "quantity-overflow"))
        (case capacity () (%string.lit "capacity"))))

    (func $base (param $op $string) (param $key $string) (result (ref $json-object))
      (%effect reads writes allocates)
      (%let $o (call $json.object-builder.new)
        (block
          (result (ref $json-object))
          (call $json.object-builder.string (local.get $o) (%string.lit "op") (local.get $op))
          (call $json.object-builder.string (local.get $o) (%string.lit "key") (local.get $key))
          (local.get $o))))

    (func $success (param $op $string) (param $key $string) (param $quantity i64)
      (result (%inst $result $unit i32)) (%effect reads writes allocates host suspends)
      (%let $o (call $base (local.get $op) (local.get $key))
        (block
          (result (%inst $result $unit i32))
          (call $json.object-builder.boolean (local.get $o) (%string.lit "ok") (i32.const 1))
          (call $json.object-builder.u64 (local.get $o) (%string.lit "quantity") (local.get $quantity))
          (%future.await
            (%inst $result $unit i32)
            (call $write (call $json.object-builder.finish (local.get $o))))
        )))

    (func $failure (param $op $string) (param $key $string) (param $error (ref $stock.Error))
      (result (%inst $result $unit i32)) (%effect reads writes allocates host suspends)
      (%let $o (call $base (local.get $op) (local.get $key))
        (block
          (result (%inst $result $unit i32))
          (call $json.object-builder.boolean (local.get $o) (%string.lit "ok") (i32.const 0))
          (call $json.object-builder.string
            (local.get $o)
            (%string.lit "error")
            (call $error-name (local.get $error)))
          (%future.await
            (%inst $result $unit i32)
            (call $write (call $json.object-builder.finish (local.get $o))))
        )))

    (func $invalid (result (%inst $result $unit i32)) (%effect host suspends)
      (%future.await
        (%inst $result $unit i32)
        (call $write (%string.lit "{\"ok\":false,\"error\":\"invalid-command\"}"))))
  )

  (import "wlt:host/text-io@0.1.0" "read-lines"
    (func $read-lines (result (%inst $stream (%inst $result $string i32))))
  )
  (%namespace $app
    (%import "wlt:std"
      (type $result)
      (type $string)
      (type $unit))
    (%public (func $process))
    (func $process (param $s (ref $stock.State)) (param $line $string)
      (result (%inst $result $unit i32)) (%effect reads writes allocates host suspends)
      (%match $parse.Command (result (%inst $result $unit i32))
        (call $parse.command (local.get $line))
        (case skip () (%result.ok $unit i32))
        (case invalid () (call $report.invalid))
        (case add ($key $n)
          (%match (%inst $result i64 (ref $stock.Error)) (result (%inst $result $unit i32))
            (call $stock.add (local.get $s) (local.get $key) (local.get $n))
            (case ok ($quantity)
              (call $report.success
                (%string.lit "ADD")
                (local.get $key)
                (local.get $quantity)))
            (case err ($error)
              (call $report.failure
                (%string.lit "ADD")
                (local.get $key)
                (local.get $error))))
        )
        (case take ($key $n)
          (%match (%inst $result i64 (ref $stock.Error)) (result (%inst $result $unit i32))
            (call $stock.take (local.get $s) (local.get $key) (local.get $n))
            (case ok ($quantity)
              (call $report.success
                (%string.lit "TAKE")
                (local.get $key)
                (local.get $quantity)))
            (case err ($error)
              (call $report.failure
                (%string.lit "TAKE")
                (local.get $key)
                (local.get $error))))
        )
        (case show ($key)
          (call $report.success
            (%string.lit "SHOW")
            (local.get $key)
            (call $stock.get (local.get $s) (local.get $key))))
      )))

  ;; 成功値は0。I/O errは伝播し、取消しの終了確認はtask scopeが担当する。
  (func $main
    (export "main")
    (result (%inst $result i32 i32))
    (%effect reads writes allocates host suspends)
    (%let
      (($initial (call $stock.new (i32.const 4096)))
       ($input (call $read-lines)))
      (%result.let ($unused-state)
        (%stream.fold-result
          (%move $input)
          (local.get $initial)
          ($state $line)
          (%result.let ()
            (call $app.process (local.get $state) (local.get $line))
            (%result.ok (local.get $state))))

        (%result.ok (i32.const 0))))
  )
(%file.namespace $wlt-file-0

  (%import "wlt:std" (type $bytes) (type $option) (type $tuple))
  (%export
    (func $bytes.concat)
    (func $bytes.equal)
    (func $bytes.find-byte)
    (func $bytes.find)
    (func $bytes.split-once)
    (func $bytes.parse-u64))
  (func $bytes.equal
    (param $a $bytes)
    (param $b $bytes)
    (result i32)
    (%effect pure)
    (if
      (result i32)
      (!= (%bytes.len $a) (%bytes.len $b))
      (then 0)
      (else
        (%loop $compare
          (bind $i i32 0)
          (result i32)
          (if
            (result i32)
            (= $i (%bytes.len $a))
            (then 1)
            (else
              (if
                (result i32)
                (!= (%bytes.get $a $i) (%bytes.get $b $i))
                (then 0)
                (else (%recur $compare (+ $i 1))))
            ))))
    ))
  ;; Return the length when absent or start is out of range. byte is unsigned.
  (func $bytes.find-byte
    (param $b $bytes)
    (param $start i32)
    (param $byte i32)
    (result i32)
    (%effect pure)
    (%loop $scan
      (bind $i i32 $start)
      (result i32)
      (if
        (result i32)
        (i32.ge_u $i (%bytes.len $b))
        (then (%bytes.len $b))
        (else
          (if
            (result i32)
            (= (%bytes.get $b $i) $byte)
            (then $i)
            (else (%recur $scan (+ $i 1))))
        ))))

  (func $bytes.matches
    (param $b $bytes)
    (param $needle $bytes)
    (param $start i32)
    (result i32)
    (%effect pure)
    (%loop $scan
      (bind $i i32 0)
      (result i32)
      (if
        (result i32)
        (= $i (%bytes.len $needle))
        (then 1)
        (else
          (if
            (result i32)
            (!= (%bytes.get $b (+ $start $i)) (%bytes.get $needle $i))
            (then 0)
            (else (%recur $scan (+ $i 1))))
        ))))

  (func $bytes.find
    (param $b $bytes)
    (param $needle $bytes)
    (param $start i32)
    (result (%inst $option i32))
    (%effect pure)
    (if
      (result (%inst $option i32))
      (i32.gt_u $start (%bytes.len $b))
      (then (%option.none))
      (else
        (if
          (result (%inst $option i32))
          (i32.gt_u (%bytes.len $needle) (- (%bytes.len $b) $start))
          (then (%option.none))
          (else
            (%let $last (- (%bytes.len $b) (%bytes.len $needle))
              (%loop $scan
                (bind $i i32 $start)
                (result (%inst $option i32))
                (if
                  (result (%inst $option i32))
                  (call $bytes.matches $b $needle $i)
                  (then (%option.some $i))
                  (else
                    (if
                      (result (%inst $option i32))
                      (= $i $last)
                      (then (%option.none))
                      (else (%recur $scan (+ $i 1))))
                  ))))
          ))))
  )

  (func $bytes.split-once
    (param $b $bytes)
    (param $needle $bytes)
    (result (%inst $option (%inst $tuple $bytes $bytes)))
    (%effect pure)
    (%match (call $bytes.find $b $needle 0)
      (case none () (%option.none))
      (case some ($i)
        (%let $next (+ $i (%bytes.len $needle))
          (%option.some
            (%tuple.new
              (%bytes.slice $b 0 $i)
              (%bytes.slice $b $next (- (%bytes.len $b) $next))))
        ))))
  ;; ASCII decimal over a checked range; test the cutoff before multiplication.
  (func $bytes.parse-u64
    (param $b $bytes)
    (param $start i32)
    (param $length i32)
    (param $limit i64)
    (result (%inst $option i64))
    (%effect pure)
    (if
      (result (%inst $option i64))
      (| (i32.eqz $length) (i32.gt_u $start (%bytes.len $b)))
      (then (%option.none))
      (else
        (if
          (result (%inst $option i64))
          (i32.gt_u $length (- (%bytes.len $b) $start))
          (then (%option.none))
          (else
            (%let
              (($cutoff (i64.div_u $limit 10))
               ($last (i32.wrap_i64 (i64.rem_u $limit 10)))
               ($end (+ $start $length)))
              (%loop $digits
                (bind $i i32 $start)
                (bind $n i64 0)
                (result (%inst $option i64))
                (if
                  (result (%inst $option i64))
                  (= $i $end)
                  (then (%option.some $n))
                  (else
                    (%let $d (- (%bytes.get $b $i) 48)
                      (if
                        (result (%inst $option i64))
                        (|
                          (i32.gt_u $d 9)
                          (|
                            (i64.gt_u $n $cutoff)
                            (& (= $n $cutoff) (i32.gt_u $d $last))))
                        (then (%option.none))
                        (else
                          (%recur $digits (+ $i 1) (+ (* $n 10) (i64.extend_i32_u $d))))
                      ))))
              ))))
      )))
  (type $Array (array (mut i32)))
  (func $bytes.concat
    (param $a $bytes)
    (param $b $bytes)
    (result $bytes)
    (%effect reads writes allocates)
    (%let $n (+ (%bytes.len $a) (%bytes.len $b))
      (if
        (result $bytes)
        (i32.lt_u $n (%bytes.len $a))
        (then (unreachable))
        (else
          (%let $out (array.new $Array 0 $n)
            (%loop $copy
              (bind $i i32 0)
              (result $bytes)
              (if
                (result $bytes)
                (= $i $n)
                (then (%bytes.from-array $Array $out))
                (else
                  (block
                    (result $bytes)
                    (array.set $Array $out $i
                      (if
                        (result i32)
                        (i32.lt_u $i (%bytes.len $a))
                        (then (%bytes.get $a $i))
                        (else (%bytes.get $b (- $i (%bytes.len $a))))
                      ))
                    (%recur $copy (+ $i 1))))
              ))))
      )))
)

(%file.namespace $wlt-file-1

  (%import "wlt:std" (type $string) (type $bytes))
  (%export (func $json-string.quote))
  (type $Array (array (mut i32)))
  (func $json-string.valid (param $bytes $bytes) (result $string) (%effect pure)
    (%match (%string.from-bytes $bytes)
      (case none () (unreachable))
      (case some ($text) $text)))
  (func $json-string.width
    (param $b i32)
    (result i32)
    (%effect pure)
    (if (result i32) (i32.lt_u $b 32)
      (then 6)
      (else (if (result i32) (| (= $b 34) (= $b 92)) (then 2) (else 1)))))

  (func $json-string.hex
    (param $n i32)
    (result i32)
    (%effect pure)
    (if (result i32) (i32.lt_u $n 10) (then (+ $n 48)) (else (+ $n 87))))

  (func $json-string.put
    (param $out (ref $Array))
    (param $i i32)
    (param $b i32)
    (result i32)
    (%effect writes)
    (%let $w (call $json-string.width $b)
      (block
        (result i32)
        (if
          (= $w 1)
          (then (array.set $Array $out $i $b))
          (else
            (block
              (array.set $Array $out $i 92)
              (if
                (= $w 2)
                (then (array.set $Array $out (+ $i 1) $b))
                (else
                  (block
                    (array.set $Array $out (+ $i 1) 117)
                    (array.set $Array $out (+ $i 2) 48)
                    (array.set $Array $out (+ $i 3) 48)
                    (array.set $Array $out (+ $i 4) (call $json-string.hex (>>> $b 4)))
                    (array.set $Array $out (+ $i 5) (call $json-string.hex (& $b 15))))
                ))))
        )
        (+ $i $w))))

  ;; Prefix bytes before first are already known not to expand. Continue the
  ;; original sizing algorithm there, so late escapes do not rescan the prefix.
  (func $json-string.quote-escaped
    (param $b $bytes)
    (param $first i32)
    (param $length i32)
    (result $string)
    (%effect reads writes allocates)
    (%let
      (($size
        (%loop $size
          (bind $i i32 $first)
          (bind $n i32 (+ $first 2))
          (result i32)
          (if (result i32) (= $i $length)
            (then $n)
            (else
              (%let $w (call $json-string.width (%bytes.get $b $i))
                (if (result i32) (i32.gt_u $n (- -1 $w))
                  (then (unreachable))
                  (else (%recur $size (+ $i 1) (+ $n $w)))))))))
       ($out (array.new $Array 34 $size)))
      (%loop $copy
        (bind $i i32 0)
        (bind $p i32 1)
        (result $string)
        (if (result $string) (= $i $length)
          (then (call $json-string.valid (%bytes.from-array $Array $out)))
          (else (%recur $copy (+ $i 1) (call $json-string.put $out $p (%bytes.get $b $i))))))))

  (func $json-string.quote
    (param $s $string)
    (result $string)
    (%effect reads writes allocates)
    (%let (($b (%string.bytes $s)) ($length (%bytes.len $b)))
      (if (result $string) (i32.gt_u $length -3)
        (then (unreachable))
        (else
          (%let $first
            (%loop $scan
              (bind $i i32 0)
              (result i32)
              (if (result i32) (= $i $length)
                (then $i)
                (else
                  (%let $byte (%bytes.get $b $i)
                    (if (result i32)
                      (| (i32.lt_u $byte 32) (| (= $byte 34) (= $byte 92)))
                      (then $i)
                      (else (%recur $scan (+ $i 1))))))))
            (if (result $string) (!= $first $length)
              (then (call $json-string.quote-escaped $b $first $length))
              ;; No escape expansion: copy directly, without width/put calls.
              (else
                (%let $out (array.new $Array 34 (+ $length 2))
                  (block (result $string)
                    (%for $copy
                      (bind $i i32 0)
                      (while (i32.lt_u $i $length))
                      (step (+ $i 1))
                      (array.set $Array $out (+ $i 1) (%bytes.get $b $i)))
                    (call $json-string.valid (%bytes.from-array $Array $out)))))))))))

)

(%file.namespace $wlt-file-2

  (%import "wlt:std" (type $bytes) (type $option) (type $tuple) (type $string))
  
(%use $wlt-file-0 (func $bytes.concat)
    (func $bytes.find)
    (func $bytes.parse-u64)
    (func $bytes.split-once))

  (%export
    (func $string.concat)
    (func $string.u64)
    (func $string.s64)
    (func $string.find)
    (func $string.split-once)
    (func $string.parse-u64)
    (func $string.parse-s64))
  (func $string.find
    (param $s $string)
    (param $needle $string)
    (param $start i32)
    (result (%inst $option i32))
    (%effect pure)
    (%let $b (%string.bytes $s)
      (if
        (result (%inst $option i32))
        (i32.gt_u $start (%bytes.len $b))
        (then (%option.none))
        (else
          (if
            (result (%inst $option i32))
            (if
              (result i32)
              (= $start (%bytes.len $b))
              (then 0)
              (else (= (& (%bytes.get $b $start) 192) 128)))
            (then (%option.none))
            (else (call $bytes.find $b (%string.bytes $needle) $start))))
      )))

  (func $string.split-once
    (param $s $string)
    (param $needle $string)
    (result (%inst $option (%inst $tuple $string $string)))
    (%effect pure)
    (%match (call $bytes.split-once (%string.bytes $s) (%string.bytes $needle))
      (case none () (%option.none))
      (case some ($parts)
        (%let ($a $b) $parts
          (%option.some (%tuple.new (call $string.valid $a) (call $string.valid $b))))
      )))

  (func $string.parse-u64
    (param $s $string)
    (result (%inst $option i64))
    (%effect pure)
    (call $bytes.parse-u64 (%string.bytes $s) 0 (%bytes.len (%string.bytes $s)) -1))

  (func $string.parse-s64
    (param $s $string)
    (result (%inst $option i64))
    (%effect pure)
    (%let $b (%string.bytes $s)
      (if
        (result (%inst $option i64))
        (i32.eqz (%bytes.len $b))
        (then (%option.none))
        (else
          (%let $negative (= (%bytes.get $b 0) 45)
            (%match
              (call $bytes.parse-u64 $b $negative
                (- (%bytes.len $b) $negative)
                ;; The negative magnitude may reach 2^63; a positive value may not.
                (if
                  (result i64)
                  $negative
                  (then -9223372036854775808)
                  (else 9223372036854775807)))
              (case none () (%option.none))
              (case some ($n)
                (%option.some (if (result i64) $negative (then (- 0 $n)) (else $n))))
            ))))
    ))
  (type $Array (array (mut i32)))
  (func $string.valid
    (param $b $bytes)
    (result $string)
    (%effect pure)
    (%match (%string.from-bytes $b)
      (case none () (unreachable))
      (case some ($s) $s)))

  (func $string.concat
    (param $a $string)
    (param $b $string)
    (result $string)
    (%effect reads writes allocates)
    (call $string.valid (call $bytes.concat (%string.bytes $a) (%string.bytes $b))))

  (func $string.u64
    (param $n i64)
    (result $string)
    (%effect reads writes allocates)
    (%let $out (array.new $Array 48 20)
      (%loop $digits
        (bind $n i64 $n)
        (bind $i i32 19)
        (result $string)
        (block
          (result $string)
          (array.set $Array $out $i (+ 48 (i32.wrap_i64 (i64.rem_u $n 10))))
          (if
            (result $string)
            (i64.lt_u $n 10)
            (then
              (call $string.valid
                (%bytes.slice (%bytes.from-array $Array $out) $i (- 20 $i))))
            (else (%recur $digits (i64.div_u $n 10) (- $i 1))))
        ))))

  (func $string.s64
    (param $n i64)
    (result $string)
    (%effect reads writes allocates)
    (if
      (result $string)
      (< $n 0)
      (then (call $string.concat (%string.lit "-") (call $string.u64 (- 0 $n))))
      (else (call $string.u64 $n))))


)

(%file.namespace $wlt-file-3

  (%import "wlt:std" (type $string))
  
(%use $wlt-file-2 (func $string.s64) (func $string.u64))

  (%export
    (opaque $string-builder)
    (func $string-builder.new)
    (func $string-builder.text)
    (func $string-builder.u64)
    (func $string-builder.s64)
    (func $string-builder.finish))
  (type $Chunk (struct (field $text $string) (field $previous (ref null $Chunk))))
  (type $string-builder
    (struct (field $head (mut (ref null $Chunk))) (field $length (mut i32))))
  (type $Array (array (mut i32)))
  (func $string-builder.new
    (result (ref $string-builder))
    (%effect allocates)
    (struct.new $string-builder (ref.null $Chunk) 0))

  (func $string-builder.text
    (param $b (ref $string-builder))
    (param $s $string)
    (%effect reads writes allocates)
    (%let $n (%bytes.len (%string.bytes $s))
      (if $n
        (then
          (%let $previous (struct.get $string-builder $length $b)
            (if
              (i32.gt_u $n (- -1 $previous))
              (then (unreachable))
              (else
                (block
                  (struct.set $string-builder $head $b
                    (struct.new $Chunk $s (struct.get $string-builder $head $b)))
                  (struct.set $string-builder $length $b (+ $previous $n))))
            )))
        (else (block))))
  )

  (func $string-builder.u64
    (param $b (ref $string-builder))
    (param $n i64)
    (%effect reads writes allocates)
    (call $string-builder.text $b (call $string.u64 $n)))

  (func $string-builder.s64
    (param $b (ref $string-builder))
    (param $n i64)
    (%effect reads writes allocates)
    (call $string-builder.text $b (call $string.s64 $n)))
  ;; Traverse backwards, placing each fragment at its final offset. No recursion.
  (func $string-builder.finish
    (param $b (ref $string-builder))
    (result $string)
    (%effect reads writes allocates)
    (%let
      (($length (struct.get $string-builder $length $b))
       ($out (array.new $Array 0 $length)))
      (block
        (result $string)
        (%loop $chunks
          (bind $chunk (ref null $Chunk) (struct.get $string-builder $head $b))
          (bind $end i32 $length)
          (if
            (ref.is_null $chunk)
            (then (block))
            (else
              (%let
                (($chunk (ref.as_non_null $chunk))
                 ($bytes (%string.bytes (struct.get $Chunk $text $chunk)))
                 ($start (- $end (%bytes.len $bytes))))
                (block
                  (%for $copy
                    (bind $i i32 0)
                    (while (i32.lt_u $i (%bytes.len $bytes)))
                    (step (+ $i 1))
                    (array.set $Array $out (+ $start $i) (%bytes.get $bytes $i)))
                  (%recur $chunks (struct.get $Chunk $previous $chunk) $start))))
          ))
        (struct.set $string-builder $head $b (ref.null $Chunk))
        (struct.set $string-builder $length $b 0)
        (%match (%string.from-bytes (%bytes.from-array $Array $out))
          (case none () (unreachable))
          (case some ($s) $s))))
  )
)

(%file.namespace $wlt-file-4

  
(%use $wlt-file-1 (func $json-string.quote))

  (%import "wlt:std"
    (type $bytes)
    (type $option)
    (type $tuple)
    (type $result)
    (type $string))
  
(%use $wlt-file-3 (type $string-builder)
    (func $string-builder.new)
    (func $string-builder.text)
    (func $string-builder.finish))

  
(%use $wlt-file-0 (func $bytes.equal))

  (%export
    (opaque $json-value)
    (type $json-kind)
    (func $json.parse)
    (func $json.stringify)
    (func $json.kind)
    (func $json.null)
    (func $json.boolean)
    (func $json.number)
    (func $json.from-string)
    (func $json.array)
    (func $json.object)
    (func $json.prepend)
    (func $json.set)
    (func $json.get)
    (func $json.at)
    (func $json.as-boolean)
    (func $json.as-number)
    (func $json.as-string))
  (type $json-kind (%variant (null) (boolean) (number) (string) (array) (object)))
  (type $json-value
    (%variant
      (null)
      (boolean i32)
      (number $string)
      (string $string)
      (array (ref null $Items))
      (object (ref null $Members))))
  (type $Items
    (struct (field $value (ref $json-value)) (field $next (ref null $Items))))
  (type $Members
    (struct
      (field $key $string)
      (field $value (ref $json-value))
      (field $next (ref null $Members))))
  ;; Result aliases keep cursor movement separate from AST construction.
  (%type.alias $Parsed (%inst $tuple (ref $json-value) i32))
  (%type.alias $ParseResult (%inst $result $Parsed i32))
  (%type.alias $OffsetResult (%inst $result i32 i32))
  (%type.alias $StringResult (%inst $result (%inst $tuple $string i32) i32))
  (func $json.peek
    (param $b $bytes)
    (param $p i32)
    (result i32)
    (%effect reads writes allocates)
    (if
      (result i32)
      (i32.lt_u $p (call $json.bytes-len $b))
      (then (%bytes.get $b $p))
      (else -1)))

  (func $json.bytes-len
    (param $b $bytes)
    (result i32)
    (%effect reads writes allocates)
    (%bytes.len $b))
  ;; Eager predicate masks keep the tuned scans branchless.
  ;; Unsigned comparisons also preserve the -1 sentinel returned by peek.
  (func $json.space
    (param $b $bytes)
    (param $start i32)
    (result i32)
    (%effect reads writes allocates)
    (%let $length (%bytes.len $b)
      (%loop $ws
        (bind $p i32 $start)
        (result i32)
        (block
          (result i32)
          (if (i32.ge_u $p $length) (then (return $p)) (else (nop)))
          (%let $c (%bytes.get $b $p)
            (if
              (result i32)
              (| (= $c 32) (| (= $c 9) (| (= $c 10) (= $c 13))))
              (then (%recur $ws (+ $p 1)))
              (else $p))))
      )))

  (func $json.digits
    (param $b $bytes)
    (param $start i32)
    (result i32)
    (%effect reads writes allocates)
    (%let $length (%bytes.len $b)
      (%loop $digits
        (bind $p i32 $start)
        (result i32)
        (block
          (result i32)
          (if (i32.ge_u $p $length) (then (return $p)) (else (nop)))
          (%let $c (%bytes.get $b $p)
            (if
              (result i32)
              (i32.le_u (- $c 48) 9)
              (then (%recur $digits (+ $p 1)))
              (else $p))))
      )))

  (func $json.slice-text
    (param $b $bytes)
    (param $start i32)
    (param $end i32)
    (result $string)
    (%effect reads writes allocates)
    (%match (%string.from-bytes (%bytes.slice $b $start (- $end $start)))
      (case some ($s) $s)
      (case none () (unreachable))))

  (func $json.hex-digit
    (param $x i32)
    (result i32)
    (%effect reads writes allocates)
    (if
      (result i32)
      (& (i32.ge_u $x 48) (i32.le_u $x 57))
      (then (- $x 48))
      (else
        (if
          (result i32)
          (& (i32.ge_u $x 65) (i32.le_u $x 70))
          (then (- $x 55))
          (else
            (if
              (result i32)
              (& (i32.ge_u $x 97) (i32.le_u $x 102))
              (then (- $x 87))
              (else -1))))
      )))

  (func $json.hex4
    (param $b $bytes)
    (param $start i32)
    (result (%inst $result i32 i32))
    (%effect reads writes allocates)
    (%loop $hex
      (bind $p i32 $start)
      (bind $n i32 0)
      (result (%inst $result i32 i32))
      (if
        (result (%inst $result i32 i32))
        (= $p (+ $start 4))
        (then (%result.ok $n))
        (else
          (%let $d (call $json.hex-digit (call $json.peek $b $p))
            (if
              (result (%inst $result i32 i32))
              (< $d 0)
              (then (%result.err $p))
              (else (%recur $hex (+ $p 1) (+ (* $n 16) $d))))
          ))))
  )

  (func $json.codepoint
    (param $n i32)
    (result $string)
    (%effect reads writes allocates)
    (%match
      (%string.from-bytes
        (if
          (result $bytes)
          (i32.lt_u $n 128)
          (then (%bytes.new $n))
          (else
            (if
              (result $bytes)
              (i32.lt_u $n 2048)
              (then (%bytes.new (| 192 (>>> $n 6)) (| 128 (& (>>> $n 0) 63))))
              (else
                (if
                  (result $bytes)
                  (i32.lt_u $n 65536)
                  (then
                    (%bytes.new
                      (| 224 (>>> $n 12))
                      (| 128 (& (>>> $n 6) 63))
                      (| 128 (& (>>> $n 0) 63))))
                  (else
                    (%bytes.new
                      (| 240 (>>> $n 18))
                      (| 128 (& (>>> $n 12) 63))
                      (| 128 (& (>>> $n 6) 63))
                      (| 128 (& (>>> $n 0) 63))))
                ))))
        ))
      (case some ($s) $s)
      (case none () (unreachable))))

  (func $json.escape-code
    (param $c i32)
    (result i32)
    (%effect pure)
    (block
      (result i32)
      (if (= $c 116) (then (return 9)) (else (nop)))
      (if (= $c 114) (then (return 13)) (else (nop)))
      (if (= $c 110) (then (return 10)) (else (nop)))
      (if (= $c 102) (then (return 12)) (else (nop)))
      (if (= $c 98) (then (return 8)) (else (nop)))
      (if (| (= $c 47) (| (= $c 92) (= $c 34))) (then (return $c)) (else (nop)))
      -1))

  (func $json.escape-unicode
    (param $b $bytes)
    (param $p i32)
    (result $StringResult)
    (%effect reads writes allocates)
    (%result.do
      ($high (call $json.hex4 $b (+ $p 2)))
      (block
        (result $StringResult)
        (if
          (& (i32.ge_u $high 56320) (i32.le_u $high 57343))
          (then (return (%result.err $p)))
          (else (nop)))
        (if
          (| (i32.lt_u $high 55296) (i32.gt_u $high 56319))
          (then
            (return
              (%result.ok (%tuple.new (call $json.codepoint $high) (+ $p 6))))
          )
          (else (nop)))
        (if
          (|
            (!= (call $json.peek $b (+ $p 6)) 92)
            (!= (call $json.peek $b (+ $p 7)) 117))
          (then (return (%result.err $p)))
          (else (nop)))
        (%result.do
          ($low (call $json.hex4 $b (+ $p 8)))
          (if
            (result $StringResult)
            (& (i32.ge_u $low 56320) (i32.le_u $low 57343))
            (then
              (%result.ok
                (%tuple.new
                  (call $json.codepoint
                    (+ 65536 (+ (* (- $high 55296) 1024) (- $low 56320))))
                  (+ $p 12))))
            (else (%result.err $p))))
      )))

  (func $json.escape
    (param $b $bytes)
    (param $p i32)
    (result $StringResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b (+ $p 1))
      (block
        (result $StringResult)
        (if
          (= $c 117)
          (then (return (call $json.escape-unicode $b $p)))
          (else (nop)))
        (%let $code (call $json.escape-code $c)
          (if
            (result $StringResult)
            (< $code 0)
            (then (%result.err $p))
            (else (%result.ok (%tuple.new (call $json.codepoint $code) (+ $p 2))))
          ))))
  )
  ;; Scan up to eight bytes per iteration. Each load is in bounds;
  ;; return at the first quote, escape or control byte.
  (func $json.string-end
    (param $b $bytes)
    (param $start i32)
    (result i32)
    (%effect reads writes allocates)
    (%let $length (%bytes.len $b)
      (%loop $scan
        (bind $p i32 $start)
        (result i32)
        (block
          (result i32)
          (if (i32.ge_u $p $length) (then (return $p)) (else (nop)))
          (if
            (result i32)
            (i32.ge_u (- $length $p) 8)
            (then
              (block
                (result i32)
                (%let $c (%bytes.get $b $p)
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return $p))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 1))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 1)))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 2))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 2)))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 3))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 3)))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 4))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 4)))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 5))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 5)))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 6))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 6)))
                    (else (nop))))
                (%let $c (%bytes.get $b (+ $p 7))
                  (if
                    (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                    (then (return (+ $p 7)))
                    (else (nop))))
                (%recur $scan (+ $p 8))))
            (else
              (%let $c (%bytes.get $b $p)
                (if
                  (result i32)
                  (| (i32.lt_u $c 32) (| (= $c 34) (= $c 92)))
                  (then $p)
                  (else (%recur $scan (+ $p 1))))
              ))))
      )))

  (func $json.parse-string
    (param $b $bytes)
    (param $quote i32)
    (result $StringResult)
    (%effect reads writes allocates)
    ;; Most strings never need an escape builder or its second copy.
    (%loop $string
      (bind $start i32 (+ $quote 1))
      (bind $out (ref null $string-builder) (ref.null $string-builder))
      (result $StringResult)
      (%let
        (($end (call $json.string-end $b $start))
         ($c (call $json.peek $b $end)))
        (block
          (result $StringResult)
          (if
            (| (= $c -1) (i32.lt_u $c 32))
            (then (return (%result.err $end)))
            (else (nop)))
          (if
            (= $c 34)
            (then
              (return
                (%result.ok
                  (%tuple.new
                    (if
                      (result $string)
                      (ref.is_null $out)
                      (then (call $json.slice-text $b $start $end))
                      (else
                        (%let $builder (ref.as_non_null $out)
                          (block
                            (result $string)
                            (call $string-builder.text $builder
                              (call $json.slice-text $b $start $end))
                            (call $string-builder.finish $builder))))
                    )
                    (+ $end 1))))
            )
            (else (nop)))
          (%result.do
            ($escaped (call $json.escape $b $end))
            (%let ($text $next) $escaped
              (%let $builder
                (if (result (ref $string-builder)) (ref.is_null $out) (then (call $string-builder.new)) (else (ref.as_non_null $out)))
                (block
                  (result $StringResult)
                  (call $string-builder.text $builder
                    (call $json.slice-text $b $start $end))
                  (call $string-builder.text $builder $text)
                  (%recur $string $next $builder))))
          ))))
  )

  (func $json.required-digits
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b $start)
      (if
        (result $OffsetResult)
        (& (i32.ge_u $c 48) (i32.le_u $c 57))
        (then (%result.ok (call $json.digits $b $start)))
        (else (%result.err $start))))
  )

  (func $json.integer-end
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (if
      (result $OffsetResult)
      (= (call $json.peek $b $start) 48)
      (then (%result.ok (+ $start 1)))
      (else (call $json.required-digits $b $start))))

  (func $json.fraction-end
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (if
      (result $OffsetResult)
      (= (call $json.peek $b $start) 46)
      (then (call $json.required-digits $b (+ $start 1)))
      (else (%result.ok $start))))

  (func $json.exponent-end
    (param $b $bytes)
    (param $start i32)
    (result $OffsetResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b $start)
      (block
        (result $OffsetResult)
        (if
          (i32.eqz (| (= $c 101) (= $c 69)))
          (then (return (%result.ok $start)))
          (else (nop)))
        (%let
          (($sign (+ $start 1))
           ($c (call $json.peek $b $sign))
           ($digits
             (if (result i32) (| (= $c 43) (= $c 45)) (then (+ $sign 1)) (else $sign))))
          (call $json.required-digits $b $digits))))
  )

  (func $json.parse-number
    (param $b $bytes)
    (param $start i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $sign
      (if (result i32) (= (call $json.peek $b $start) 45) (then (+ $start 1)) (else $start))
      (%result.do
        ($integer (call $json.integer-end $b $sign))
        ($fraction (call $json.fraction-end $b $integer))
        ($end (call $json.exponent-end $b $fraction))
        (%result.ok
          (%tuple.new
            (%variant.new $json-value number
              (call $json.slice-text $b $start $end))
            $end))))
  )

  (func $json.literal
    (param $b $bytes)
    (param $p i32)
    (param $literal $string)
    (result (%inst $result i32 i32))
    (%effect reads writes allocates)
    (%let
      (($word (%string.bytes $literal))
       ($n (%bytes.len $word)))
      (if
        (result (%inst $result i32 i32))
        (i32.le_u $n (- (%bytes.len $b) $p))
        (then
          (if
            (result (%inst $result i32 i32))
            (call $bytes.equal (%bytes.slice $b $p $n) $word)
            (then (%result.ok (+ $p $n)))
            (else (%result.err $p))))
        (else (%result.err $p))))
  )

  (func $json.reverse-items
    (param $head (ref null $Items))
    (param $tail (ref null $Items))
    (result (ref null $Items))
    (%effect reads writes allocates)
    (%loop $reverse
      (bind $input (ref null $Items) $head)
      (bind $out (ref null $Items) $tail)
      (result (ref null $Items))
      (if
        (result (ref null $Items))
        (ref.is_null $input)
        (then $out)
        (else
          (%let $node (ref.as_non_null $input)
            (%recur $reverse
              (struct.get $Items $next $node)
              (struct.new $Items (struct.get $Items $value $node) $out))))
      )))

  (func $json.reverse-members
    (param $head (ref null $Members))
    (param $tail (ref null $Members))
    (result (ref null $Members))
    (%effect reads writes allocates)
    (%loop $reverse
      (bind $input (ref null $Members) $head)
      (bind $out (ref null $Members) $tail)
      (result (ref null $Members))
      (if
        (result (ref null $Members))
        (ref.is_null $input)
        (then $out)
        (else
          (%let $node (ref.as_non_null $input)
            (%recur $reverse
              (struct.get $Members $next $node)
              (struct.new $Members
                (struct.get $Members $key $node)
                (struct.get $Members $value $node)
                $out))))
      )))

  (func $json.parse-array
    (param $b $bytes)
    (param $open i32)
    (param $depth i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $start (call $json.space $b (+ $open 1))
      (block
        (result $ParseResult)
        (if
          (= (call $json.peek $b $start) 93)
          (then
            (return
              (%result.ok
                (%tuple.new
                  (%variant.new $json-value array (ref.null $Items))
                  (+ $start 1))))
          )
          (else (nop)))
        ;; Siblings are iterative. Recursion follows nested values only.
        (%loop $read-array
          (bind $p i32 $start)
          (bind $head (ref null $Items) (ref.null $Items))
          (result $ParseResult)
          (%result.do
            ($parsed (call $json.parse-value $b $p (+ $depth 1)))
            (%let ($item $next) $parsed
              (%let
                (($head (struct.new $Items $item $head))
                 ($end (call $json.space $b $next))
                 ($c (call $json.peek $b $end)))
                (block
                  (result $ParseResult)
                  (if
                    (= $c 93)
                    (then
                      (return
                        (%result.ok
                          (%tuple.new
                            (%variant.new $json-value array
                              (call $json.reverse-items $head (ref.null $Items)))
                            (+ $end 1))))
                    )
                    (else (nop)))
                  (if (!= $c 44) (then (return (%result.err $end))) (else (nop)))
                  (%recur $read-array (call $json.space $b (+ $end 1)) $head))))
          ))))
  )

  (func $json.parse-object
    (param $b $bytes)
    (param $open i32)
    (param $depth i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $start (call $json.space $b (+ $open 1))
      (block
        (result $ParseResult)
        (if
          (= (call $json.peek $b $start) 125)
          (then
            (return
              (%result.ok
                (%tuple.new
                  (%variant.new $json-value object (ref.null $Members))
                  (+ $start 1))))
          )
          (else (nop)))
        (%loop $read-object
          (bind $p i32 $start)
          (bind $head (ref null $Members) (ref.null $Members))
          (result $ParseResult)
          (block
            (result $ParseResult)
            (if
              (!= (call $json.peek $b $p) 34)
              (then (return (%result.err $p)))
              (else (nop)))
            (%result.do
              ($parsed-key (call $json.parse-string $b $p))
              (%let ($key $after-key) $parsed-key
                (%let $colon (call $json.space $b $after-key)
                  (block
                    (result $ParseResult)
                    (if
                      (!= (call $json.peek $b $colon) 58)
                      (then (return (%result.err $colon)))
                      (else (nop)))
                    (%result.do
                      ($parsed
                        (call $json.parse-value $b
                          (call $json.space $b (+ $colon 1))
                          (+ $depth 1)))
                      (%let ($item $next) $parsed
                        (%let
                          (($head (struct.new $Members $key $item $head))
                           ($end (call $json.space $b $next))
                           ($c (call $json.peek $b $end)))
                          (block
                            (result $ParseResult)
                            (if
                              (= $c 125)
                              (then
                                (return
                                  (%result.ok
                                    (%tuple.new
                                      (%variant.new $json-value object
                                        (call $json.reverse-members $head
                                          (ref.null $Members)))
                                      (+ $end 1))))
                              )
                              (else (nop)))
                            (if
                              (!= $c 44)
                              (then (return (%result.err $end)))
                              (else (nop)))
                            (%recur $read-object
                              (call $json.space $b (+ $end 1))
                              $head))))
                    ))))
            ))))
    ))

  (func $json.parse-value
    (param $b $bytes)
    (param $p i32)
    (param $depth i32)
    (result $ParseResult)
    (%effect reads writes allocates)
    (%let $c (call $json.peek $b $p)
      (block
        (result $ParseResult)
        ;; Depth counts containers, so an empty container cannot bypass it.
        (if
          (| (i32.gt_u $depth 128) (& (= $depth 128) (| (= $c 123) (= $c 91))))
          (then (return (%result.err $p)))
          (else (nop)))
        (if
          (= $c 123)
          (then (return (call $json.parse-object $b $p $depth)))
          (else (nop)))
        (if
          (= $c 91)
          (then (return (call $json.parse-array $b $p $depth)))
          (else (nop)))
        (if
          (= $c 34)
          (then
            (return
              (%result.do
                ($s (call $json.parse-string $b $p))
                (%let ($text $next) $s
                  (%result.ok
                    (%tuple.new (%variant.new $json-value string $text) $next))))
            ))
          (else (nop)))
        (if
          (= $c 102)
          (then
            (return
              (%result.do
                ($next (call $json.literal $b $p (%string.lit "false")))
                (%result.ok (%tuple.new (%variant.new $json-value boolean 0) $next))))
          )
          (else (nop)))
        (if
          (= $c 116)
          (then
            (return
              (%result.do
                ($next (call $json.literal $b $p (%string.lit "true")))
                (%result.ok (%tuple.new (%variant.new $json-value boolean 1) $next))))
          )
          (else (nop)))
        (if
          (= $c 110)
          (then
            (return
              (%result.do
                ($next (call $json.literal $b $p (%string.lit "null")))
                (%result.ok (%tuple.new (%variant.new $json-value null) $next))))
          )
          (else (nop)))
        (call $json.parse-number $b $p))))

  (func $json.parse
    (param $source $string)
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%let $b (%string.bytes $source)
      (block
        (result (%inst $result (ref $json-value) i32))
        (if
          (i32.gt_u (%bytes.len $b) 65536)
          (then (return (%result.err 65536)))
          (else (nop)))
        (%result.do
          ($parsed (call $json.parse-value $b (call $json.space $b 0) 0))
          (%let ($value $next) $parsed
            (%let $end (call $json.space $b $next)
              (if
                (result (%inst $result (ref $json-value) i32))
                (= $end (%bytes.len $b))
                (then (%result.ok $value))
                (else (%result.err $end))))
          ))))
  )

  (func $json.null
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value null))

  (func $json.boolean
    (param $value i32)
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value boolean (!= $value 0)))

  (func $json.from-string
    (param $value $string)
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value string $value))

  (func $json.array
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value array (ref.null $Items)))

  (func $json.object
    (result (ref $json-value))
    (%effect reads writes allocates)
    (%variant.new $json-value object (ref.null $Members)))

  (func $json.number
    (param $lexeme $string)
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%result.let ($value)
      (call $json.parse $lexeme)
      (%match $value
        (case null () (%result.err 0))
        (case boolean ($x) (%result.err 0))
        (case number ($x) (%result.ok $value))
        (case string ($x) (%result.err 0))
        (case array ($x) (%result.err 0))
        (case object ($x) (%result.err 0))))
  )

  (func $json.kind
    (param $value (ref $json-value))
    (result (ref $json-kind))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%variant.new $json-kind null))
      (case boolean ($x) (%variant.new $json-kind boolean))
      (case number ($x) (%variant.new $json-kind number))
      (case string ($x) (%variant.new $json-kind string))
      (case array ($x) (%variant.new $json-kind array))
      (case object ($x) (%variant.new $json-kind object))))

  (func $json.as-boolean
    (param $value (ref $json-value))
    (result (%inst $option i32))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%option.none))
      (case boolean ($x) (%option.some $x))
      (case number ($x) (%option.none))
      (case string ($x) (%option.none))
      (case array ($x) (%option.none))
      (case object ($x) (%option.none))))

  (func $json.as-number
    (param $value (ref $json-value))
    (result (%inst $option $string))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%option.none))
      (case boolean ($x) (%option.none))
      (case number ($x) (%option.some $x))
      (case string ($x) (%option.none))
      (case array ($x) (%option.none))
      (case object ($x) (%option.none))))

  (func $json.as-string
    (param $value (ref $json-value))
    (result (%inst $option $string))
    (%effect reads writes allocates)
    (%match $value
      (case null () (%option.none))
      (case boolean ($x) (%option.none))
      (case number ($x) (%option.none))
      (case string ($x) (%option.some $x))
      (case array ($x) (%option.none))
      (case object ($x) (%option.none))))

  (func $json.prepend
    (param $array (ref $json-value))
    (param $item (ref $json-value))
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%match $array
      (case null () (%result.err 0))
      (case boolean ($x) (%result.err 0))
      (case number ($x) (%result.err 0))
      (case string ($x) (%result.err 0))
      (case array ($x)
        (%result.ok (%variant.new $json-value array (struct.new $Items $item $x))))
      (case object ($x) (%result.err 0))))

  (func $json.set
    (param $object (ref $json-value))
    (param $key $string)
    (param $item (ref $json-value))
    (result (%inst $result (ref $json-value) i32))
    (%effect reads writes allocates)
    (%match $object
      (case null () (%result.err 0))
      (case boolean ($x) (%result.err 0))
      (case number ($x) (%result.err 0))
      (case string ($x) (%result.err 0))
      (case array ($x) (%result.err 0))
      (case object ($x)
        (%let $reverse (call $json.reverse-members $x (ref.null $Members))
          (%result.ok
            (%variant.new $json-value object
              (call $json.reverse-members $reverse
                (struct.new $Members $key $item (ref.null $Members))))
          ))))
  )

  (func $json.get
    (param $object (ref $json-value))
    (param $key $string)
    (result (%inst $option (ref $json-value)))
    (%effect reads writes allocates)
    (%match $object
      (case null () (%option.none))
      (case boolean ($x) (%option.none))
      (case number ($x) (%option.none))
      (case string ($x) (%option.none))
      (case array ($x) (%option.none))
      (case object ($x)
        (%loop $get
          (bind $head (ref null $Members) $x)
          (bind $found (%inst $option (ref $json-value)) (%option.none))
          (result (%inst $option (ref $json-value)))
          (if
            (result (%inst $option (ref $json-value)))
            (ref.is_null $head)
            (then $found)
            (else
              (%let $node (ref.as_non_null $head)
                (%recur $get
                  (struct.get $Members $next $node)
                  (if
                    (result (%inst $option (ref $json-value)))
                    (call $bytes.equal
                      (%string.bytes $key)
                      (%string.bytes (struct.get $Members $key $node)))
                    (then (%option.some (struct.get $Members $value $node)))
                    (else $found))))
            ))))
    ))

  (func $json.at
    (param $array (ref $json-value))
    (param $index i32)
    (result (%inst $option (ref $json-value)))
    (%effect reads writes allocates)
    (%match $array
      (case null () (%option.none))
      (case boolean ($x) (%option.none))
      (case number ($x) (%option.none))
      (case string ($x) (%option.none))
      (case array ($x)
        (%loop $at
          (bind $head (ref null $Items) $x)
          (bind $index i32 $index)
          (result (%inst $option (ref $json-value)))
          (if
            (result (%inst $option (ref $json-value)))
            (ref.is_null $head)
            (then (%option.none))
            (else
              (%let $node (ref.as_non_null $head)
                (if
                  (result (%inst $option (ref $json-value)))
                  (= $index 0)
                  (then (%option.some (struct.get $Items $value $node)))
                  (else (%recur $at (struct.get $Items $next $node) (- $index 1))))
              ))))
      )
      (case object ($x) (%option.none))))

  (func $json.write
    (param $value (ref $json-value))
    (param $out (ref $string-builder))
    (%effect reads writes allocates)
    (%match $value
      (case null () (call $string-builder.text $out (%string.lit "null")))
      (case boolean ($x)
        (call $string-builder.text $out
          (if
            (result $string)
            $x
            (then (%string.lit "true"))
            (else (%string.lit "false"))))
      )
      (case number ($x) (call $string-builder.text $out $x))
      (case string ($x) (call $string-builder.text $out (call $json-string.quote $x)))
      (case array ($x)
        (block
          (call $string-builder.text $out (%string.lit "["))
          (%loop $write-array
            (bind $head (ref null $Items) $x)
            (bind $first i32 1)
            (if
              (ref.is_null $head)
              (then (nop))
              (else
                (%let $node (ref.as_non_null $head)
                  (block
                    (if
                      (= $first 0)
                      (then (call $string-builder.text $out (%string.lit ",")))
                      (else (nop)))
                    (call $json.write (struct.get $Items $value $node) $out)
                    (%recur $write-array (struct.get $Items $next $node) 0))))
            ))
          (call $string-builder.text $out (%string.lit "]"))))
      (case object ($x)
        (block
          (call $string-builder.text $out (%string.lit "{"))
          (%loop $write-object
            (bind $head (ref null $Members) $x)
            (bind $first i32 1)
            (if
              (ref.is_null $head)
              (then (nop))
              (else
                (%let $node (ref.as_non_null $head)
                  (block
                    (if
                      (= $first 0)
                      (then (call $string-builder.text $out (%string.lit ",")))
                      (else (nop)))
                    (call $string-builder.text $out (call $json-string.quote (struct.get $Members $key $node)))
                    (call $string-builder.text $out (%string.lit ":"))
                    (call $json.write (struct.get $Members $value $node) $out)
                    (%recur $write-object (struct.get $Members $next $node) 0))))
            ))
          (call $string-builder.text $out (%string.lit "}"))))
    ))

  (func $json.stringify
    (param $value (ref $json-value))
    (result $string)
    (%effect reads writes allocates)
    (block (result $string)
      ;; A standalone JSON string needs no intermediate builder or second copy.
      (%match $value
        (case null () (nop))
        (case boolean ($x) (nop))
        (case number ($x) (nop))
        (case string ($x) (return (call $json-string.quote $x)))
        (case array ($x) (nop))
        (case object ($x) (nop)))
      (%let $out (call $string-builder.new)
        (block (result $string)
          (call $json.write $value $out)
          (call $string-builder.finish $out)))))

)

(%file.namespace $wlt-file-5

  
(%use $wlt-file-1 (func $json-string.quote))

  (%import "wlt:std" (type $result) (type $string) (type $unit))
  
(%use $wlt-file-4 (type $json-value)
    (func $json.number)
    (func $json.stringify))

  
(%use $wlt-file-3 (type $string-builder)
    (func $string-builder.finish)
    (func $string-builder.new)
    (func $string-builder.s64)
    (func $string-builder.text)
    (func $string-builder.u64))

  (%export
    (opaque $json-object)
    (func $json.object-builder.new)
    (func $json.object-builder.string)
    (func $json.object-builder.u64)
    (func $json.object-builder.s64)
    (func $json.object-builder.boolean)
    (func $json.object-builder.object)
    (func $json.object-builder.null)
    (func $json.object-builder.number)
    (func $json.object-builder.value)
    (func $json.object-builder.finish))
  (type $json-object
    (struct (field $builder (ref $string-builder)) (field $first (mut i32))))
  (func $json.object-builder.new
    (result (ref $json-object))
    (%effect reads writes allocates)
    (%let $b (call $string-builder.new)
      (block
        (result (ref $json-object))
        (call $string-builder.text $b (%string.lit "{"))
        (struct.new $json-object $b 1))))

  (func $json.object-builder.key
    (param $o (ref $json-object))
    (param $key $string)
    (%effect reads writes allocates)
    (block
      (if
        (i32.eqz (struct.get $json-object $first $o))
        (then
          (call $string-builder.text
            (struct.get $json-object $builder $o)
            (%string.lit ",")))
        (else (block)))
      (struct.set $json-object $first $o 0)
      (call $string-builder.text (struct.get $json-object $builder $o) (call $json-string.quote $key))
      (call $string-builder.text
        (struct.get $json-object $builder $o)
        (%string.lit ":"))))

  (func $json.object-builder.string
    (param $o (ref $json-object))
    (param $key $string)
    (param $value $string)
    (%effect reads writes allocates)
    (block
      (call $json.object-builder.key $o $key)
      (call $string-builder.text (struct.get $json-object $builder $o) (call $json-string.quote $value))))

  (func $json.object-builder.u64
    (param $o (ref $json-object))
    (param $key $string)
    (param $value i64)
    (%effect reads writes allocates)
    (block
      (call $json.object-builder.key $o $key)
      (call $string-builder.u64 (struct.get $json-object $builder $o) $value)))

  (func $json.object-builder.s64
    (param $o (ref $json-object))
    (param $key $string)
    (param $value i64)
    (%effect reads writes allocates)
    (block
      (call $json.object-builder.key $o $key)
      (call $string-builder.s64 (struct.get $json-object $builder $o) $value)))

  (func $json.object-builder.boolean
    (param $o (ref $json-object))
    (param $key $string)
    (param $value i32)
    (%effect reads writes allocates)
    (block
      (call $json.object-builder.key $o $key)
      (call $string-builder.text
        (struct.get $json-object $builder $o)
        (if
          (result $string)
          $value
          (then (%string.lit "true"))
          (else (%string.lit "false"))))
    ))

  (func $json.object-builder.finish
    (param $o (ref $json-object))
    (result $string)
    (%effect reads writes allocates)
    (%let $b (struct.get $json-object $builder $o)
      (block
        (result $string)
        (call $string-builder.text $b (%string.lit "}"))
        (%let $result (call $string-builder.finish $b)
          (block
            (result $string)
            (call $string-builder.text $b (%string.lit "{"))
            (struct.set $json-object $first $o 1)
            $result))))
  )

  (func $json.object-builder.object
    (param $o (ref $json-object))
    (param $key $string)
    (param $value (ref $json-object))
    (%effect reads writes allocates)
    ;; Drain the child before touching the parent: aliases cannot form cycles.
    (%let $value (call $json.object-builder.finish $value)
      (block
        (call $json.object-builder.key $o $key)
        (call $string-builder.text (struct.get $json-object $builder $o) $value))))

  (func $json.object-builder.null
    (param $o (ref $json-object))
    (param $key $string)
    (%effect reads writes allocates)
    (block
      (call $json.object-builder.key $o $key)
      (call $string-builder.text
        (struct.get $json-object $builder $o)
        (%string.lit "null"))))

  (func $json.object-builder.value
    (param $o (ref $json-object))
    (param $key $string)
    (param $value (ref $json-value))
    (%effect reads writes allocates)
    (%let $text (call $json.stringify $value)
      (block
        (call $json.object-builder.key $o $key)
        (call $string-builder.text (struct.get $json-object $builder $o) $text))))

  (func $json.object-builder.number
    (param $o (ref $json-object))
    (param $key $string)
    (param $lexeme $string)
    (result (%inst $result $unit i32))
    (%effect reads writes allocates)
    (%result.let ($value)
      (call $json.number $lexeme)
      (block
        (result (%inst $result $unit i32))
        (call $json.object-builder.value $o $key $value)
        (%result.ok))))

)
)
