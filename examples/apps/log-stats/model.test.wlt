(module
  (%import "wlt:std"
    (type $text))
  (%include "parse.wlt")
  (%namespace $stats
    (%include "stats-impl.wlt")
    (%test "sum overflow rejects the record without partial mutation"
      (%let $s (call $stats.new)
        (block
          (result i32)
          (drop
            (call $stats.consume
              (local.get $s)
              (%variant.new $parse.Line info (i64.const -1))))
          (i32.and
            (i32.eqz
              (call $stats.consume
                (local.get $s)
                (%variant.new $parse.Line info (i64.const 1))))
            (i32.and
              (i64.eq (call $stats.accepted (local.get $s)) (i64.const 1))
              (i32.and
                (i64.eq
                  (call $stats.count (local.get $s) (%variant.new $parse.Level info))
                  (i64.const 1))
                (i64.eq
                  (call $stats.total (local.get $s) (%variant.new $parse.Level info))
                  (i64.const -1))))
          ))))
    (%test "counter overflow rejects before mutation"
      (%let $s (call $stats.new)
        (block
          (result i32)
          (struct.set $stats.State $accepted (local.get $s) (i64.const -1))
          (struct.set $stats.State $rejected (local.get $s) (i64.const -1))
          (struct.set $stats.State $ignored (local.get $s) (i64.const -1))
          (i32.and
            (i32.eqz
              (call $stats.consume (local.get $s) (%variant.new $parse.Line skip)))
            (i32.and
              (i32.eqz
                (call $stats.consume (local.get $s) (%variant.new $parse.Line invalid)))
              (i32.and
                (i32.eqz
                  (call $stats.consume
                    (local.get $s)
                    (%variant.new $parse.Line info (i64.const 1))))
                (i64.eq
                  (call $stats.count (local.get $s) (%variant.new $parse.Level info))
                  (i64.const 0))))
          ))))
  )
  (%include "decimal.wlt")
  (func $level-id (param $l (ref $parse.Level)) (result i32)
    (%match $parse.Level (result i32)
      (local.get $l)
      (case info () (i32.const 0))
      (case warn () (i32.const 1))
      (case error () (i32.const 2))))

  (func $entry (param $line $text) (param $level i32) (param $ms i64) (result i32)
    (%match $parse.Line (result i32)
      (call $parse.line (local.get $line))
      (case skip () (i32.const 0))
      (case invalid () (i32.const 0))
      (case info ($n)
        (i32.and
          (i32.eq (i32.const 0) (local.get $level))
          (i64.eq (local.get $n) (local.get $ms))))
      (case warn ($n)
        (i32.and
          (i32.eq (i32.const 1) (local.get $level))
          (i64.eq (local.get $n) (local.get $ms))))
      (case error ($n)
        (i32.and
          (i32.eq (i32.const 2) (local.get $level))
          (i64.eq (local.get $n) (local.get $ms))))
    ))

  (func $invalid (param $line $text) (result i32)
    (%match $parse.Line (result i32)
      (call $parse.line (local.get $line))
      (case skip () (i32.const 0))
      (case invalid () (i32.const 1))
      (case info ($n) (i32.const 0))
      (case warn ($n) (i32.const 0))
      (case error ($n) (i32.const 0))))

  (func $skip (param $line $text) (result i32)
    (%match $parse.Line (result i32)
      (call $parse.line (local.get $line))
      (case skip () (i32.const 1))
      (case invalid () (i32.const 0))
      (case info ($n) (i32.const 0))
      (case warn ($n) (i32.const 0))
      (case error ($n) (i32.const 0))))

  (func $equal (param $a $text) (param $b $text) (result i32)
    (%let $x (%text.bytes (local.get $a))
      (%let $y (%text.bytes (local.get $b))
        (if
          (result i32)
          (i32.ne (%bytes.len (local.get $x)) (%bytes.len (local.get $y)))
          (then (i32.const 0))
          (else
            (%loop $go
              (bind $i i32 (i32.const 0))
              (result i32)
              (if
                (result i32)
                (i32.eq (local.get $i) (%bytes.len (local.get $x)))
                (then (i32.const 1))
                (else
                  (if
                    (result i32)
                    (i32.ne
                      (%bytes.get (local.get $x) (local.get $i))
                      (%bytes.get (local.get $y) (local.get $i)))
                    (then (i32.const 0))
                    (else (%recur $go (i32.add (local.get $i) (i32.const 1))))
                  ))))
          ))))
  )
  (%test "levels, Unicode messages, zero and u32 maximum"
    (i32.and
      (call $entry (%text.lit "INFO\t0\t日本語🦀") (i32.const 0) (i64.const 0))
      (i32.and
        (call $entry (%text.lit "WARN\t00042\t") (i32.const 1) (i64.const 42))
        (call $entry
          (%text.lit "ERROR\t4294967295\tmessage\twith tabs")
          (i32.const 2)
          (i64.const 4294967295))))
  )
  (%test "blank and comment lines are skipped"
    (i32.and (call $skip (%text.lit "")) (call $skip (%text.lit "# 日本語 comment"))))
  (%test "missing fields, unknown levels and invalid integers"
    (i32.and
      (call $invalid (%text.lit "INFO"))
      (i32.and
        (call $invalid (%text.lit "INFO\t1"))
        (i32.and
          (call $invalid (%text.lit "DEBUG\t1\tmsg"))
          (i32.and
            (call $invalid (%text.lit "INFO\t\tmsg"))
            (i32.and
              (call $invalid (%text.lit "INFO\t-1\tmsg"))
              (i32.and
                (call $invalid (%text.lit "INFO\t1x\tmsg"))
                (i32.and
                  (call $invalid (%text.lit "INFO\t4294967296\tmsg"))
                  (call $invalid (%text.lit "INFO\t99999999999999999999999\tmsg"))))
            ))))
    ))
  (%test "separate buckets, maxima, skipped and rejected counters"
    (%let $s (call $stats.new)
      (block
        (result i32)
        (drop
          (call $stats.consume
            (local.get $s)
            (call $parse.line (%text.lit "INFO\t10\ta"))))
        (drop
          (call $stats.consume
            (local.get $s)
            (call $parse.line (%text.lit "WARN\t20\tb"))))
        (drop
          (call $stats.consume
            (local.get $s)
            (call $parse.line (%text.lit "INFO\t5\tc"))))
        (drop (call $stats.consume (local.get $s) (call $parse.line (%text.lit "bad"))))
        (drop (call $stats.consume (local.get $s) (call $parse.line (%text.lit ""))))
        (i32.and
          (i64.eq (call $stats.accepted (local.get $s)) (i64.const 3))
          (i32.and
            (i64.eq (call $stats.rejected (local.get $s)) (i64.const 1))
            (i32.and
              (i64.eq (call $stats.ignored (local.get $s)) (i64.const 1))
              (i32.and
                (i64.eq
                  (call $stats.count (local.get $s) (%variant.new $parse.Level info))
                  (i64.const 2))
                (i32.and
                  (i64.eq
                    (call $stats.total (local.get $s) (%variant.new $parse.Level info))
                    (i64.const 15))
                  (i32.and
                    (i64.eq
                      (call $stats.maximum
                        (local.get $s)
                        (%variant.new $parse.Level info))
                      (i64.const 10))
                    (i64.eq
                      (call $stats.count
                        (local.get $s)
                        (%variant.new $parse.Level error))
                      (i64.const 0))))
              ))))
      )))
  (%test "unsigned decimal covers zero and all 64 bits"
    (i32.and
      (call $equal (call $decimal.text (i64.const 0)) (%text.lit "0"))
      (i32.and
        (call $equal (call $decimal.text (i64.const 42)) (%text.lit "42"))
        (i32.and
          (call $equal
            (call $decimal.text (i64.const 4294967296))
            (%text.lit "4294967296"))
          (call $equal
            (call $decimal.text (i64.const -1))
            (%text.lit "18446744073709551615"))))
    )))
