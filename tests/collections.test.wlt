(module
  (%import "wlt:std"
    (type $text)
    (type $option)
    (type $pair)
    (type $list)
    (func $string.u64)
    (func $buffer.equal)
    (func $dict.new)
    (func $dict.len)
    (func $dict.get)
    (func $dict.set)
    (func $dict.remove)
    (func $dict.clear)
    (func $dict.entries)
    (func $list-builder.new)
    (func $list-builder.push)
    (func $list-builder.extend)
    (func $list-builder.len)
    (func $list-builder.finish)
    (func $list-builder.clear))
  (func $value
    (param $v (%inst $option i64))
    (result i64)
    (%match (local.get $v)
      (case none () (i64.const -999))
      (case some ($n) (local.get $n))))

  (func $equal
    (param $a $text)
    (param $b $text)
    (result i32)
    (%effects pure)
    (call $buffer.equal (%text.bytes (local.get $a)) (%text.bytes (local.get $b))))
  (%test "dict collision, replacement, unlink and clear"
    (%let
      (($d (call (%inst $dict.new i64)))
       ($missing (call (%inst $dict.get i64) (local.get $d) (%text.lit "a")))
       ($a (call (%inst $dict.set i64) (local.get $d) (%text.lit "a") (i64.const 10)))
       ($q (call (%inst $dict.set i64) (local.get $d) (%text.lit "q") (i64.const 20)))
       ($bang
         (call (%inst $dict.set i64) (local.get $d) (%text.lit "!") (i64.const 30)))
       ($old (call (%inst $dict.set i64) (local.get $d) (%text.lit "q") (i64.const 21)))
       ($size (call (%inst $dict.len i64) (local.get $d)))
       ($middle (call (%inst $dict.remove i64) (local.get $d) (%text.lit "q")))
       ($head (call (%inst $dict.remove i64) (local.get $d) (%text.lit "!")))
       ($tail (call (%inst $dict.remove i64) (local.get $d) (%text.lit "a"))))
      (i32.and
        (i64.eq (call $value (local.get $missing)) (i64.const -999))
        (i32.and
          (i32.eq (local.get $size) (i32.const 3))
          (i32.and
            (i64.eq (call $value (local.get $old)) (i64.const 20))
            (i32.and
              (i64.eq (call $value (local.get $middle)) (i64.const 21))
              (i32.and
                (i64.eq (call $value (local.get $head)) (i64.const 30))
                (i32.and
                  (i64.eq (call $value (local.get $tail)) (i64.const 10))
                  (block
                    (result i32)
                    (drop
                      (call
                        (%inst $dict.set i64)
                        (local.get $d)
                        (%text.lit "again")
                        (i64.const 1)))
                    (call (%inst $dict.clear i64) (local.get $d))
                    (i32.eqz (call (%inst $dict.len i64) (local.get $d))))
                ))))
        ))))
  (%test "dict grows and preserves every key after removals"
    (%let $d (call (%inst $dict.new i64))
      (block
        (result i32)
        (%for $fill
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (i32.const 512)))
          (step (i32.add (local.get $i) (i32.const 1)))
          (drop
            (call
              (%inst $dict.set i64)
              (local.get $d)
              (call $string.u64 (i64.extend_i32_u (local.get $i)))
              (i64.extend_i32_u (local.get $i))))
        )
        (%for $remove
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (i32.const 512)))
          (step (i32.add (local.get $i) (i32.const 2)))
          (drop
            (call
              (%inst $dict.remove i64)
              (local.get $d)
              (call $string.u64 (i64.extend_i32_u (local.get $i))))
          ))
        (%loop $verify
          (bind $i i32 (i32.const 0))
          (bind $ok i32
            (i32.eq (call (%inst $dict.len i64) (local.get $d)) (i32.const 256)))
          (result i32)
          (if
            (result i32)
            (i32.eq (local.get $i) (i32.const 512))
            (then (local.get $ok))
            (else
              (%let $actual
                (call $value
                  (call
                    (%inst $dict.get i64)
                    (local.get $d)
                    (call $string.u64 (i64.extend_i32_u (local.get $i))))
                )
                (%recur $verify
                  (i32.add (local.get $i) (i32.const 1))
                  (i32.and
                    (local.get $ok)
                    (i64.eq
                      (local.get $actual)
                      (if
                        (result i64)
                        (i32.and (local.get $i) (i32.const 1))
                        (then (i64.extend_i32_u (local.get $i)))
                        (else (i64.const -999))))
                  ))))
          ))))
  )
  (%test "dict Unicode and nested option distinguish missing from stored none"
    (%let $d (call (%inst $dict.new (%inst $option $text)))
      (block
        (result i32)
        (drop
          (call
            (%inst $dict.set (%inst $option $text))
            (local.get $d)
            (%text.lit "日本語🦀\00")
            (%option.none $text)))
        (drop
          (call
            (%inst $dict.set (%inst $option $text))
            (local.get $d)
            (%text.lit "")
            (%option.some (%text.lit "empty"))))
        (i32.and
          (%match (%inst $option (%inst $option $text)) (result i32)
            (call
              (%inst $dict.get (%inst $option $text))
              (local.get $d)
              (%text.lit "日本語🦀\00"))
            (case none () (i32.const 0))
            (case some ($inner)
              (%match (local.get $inner)
                (case none () (i32.const 1))
                (case some ($s) (i32.const 0))))
          )
          (%match (%inst $option (%inst $option $text)) (result i32)
            (call (%inst $dict.get (%inst $option $text)) (local.get $d) (%text.lit ""))
            (case none () (i32.const 0))
            (case some ($inner)
              (%match (local.get $inner)
                (case none () (i32.const 0))
                (case some ($s) (call $equal (local.get $s) (%text.lit "empty"))))
            ))))
    ))
  (%test "list builder empty, order, snapshots, extend and reuse"
    (%let
      (($b (call (%inst $list-builder.new i64)))
       ($empty (call (%inst $list-builder.finish i64) (local.get $b))))
      (block
        (result i32)
        (call (%inst $list-builder.push i64) (local.get $b) (i64.const 40))
        (call (%inst $list-builder.push i64) (local.get $b) (i64.const 2))
        (%let $snapshot (call (%inst $list-builder.finish i64) (local.get $b))
          (block
            (result i32)
            (call (%inst $list-builder.push i64) (local.get $b) (i64.const 99))
            (call (%inst $list-builder.clear i64) (local.get $b))
            (call (%inst $list-builder.extend i64) (local.get $b) (local.get $snapshot))
            (%let $copy (call (%inst $list-builder.finish i64) (local.get $b))
              (i32.and
                (i32.eqz (%list.len i64 (local.get $empty)))
                (i32.and
                  (i32.eqz (call (%inst $list-builder.len i64) (local.get $b)))
                  (i32.and
                    (i32.eq (%list.len i64 (local.get $copy)) (i32.const 2))
                    (i64.eq
                      (i64.add
                        (%list.get i64 (local.get $copy) (i32.const 0))
                        (%list.get i64 (local.get $snapshot) (i32.const 1)))
                      (i64.const 42))))
              ))))
      )))
  (type $Cell (struct (field $value (mut i64))))
  (%test "shared reference payloads keep identity across collection snapshots"
    (%let
      (($cell (struct.new $Cell (i64.const 40)))
       ($dict (call (%inst $dict.new (ref $Cell))))
       ($builder (call (%inst $list-builder.new (ref $Cell))))
      )
      (block
        (result i32)
        (drop
          (call
            (%inst $dict.set (ref $Cell))
            (local.get $dict)
            (%text.lit "cell")
            (local.get $cell)))
        (call
          (%inst $list-builder.push (ref $Cell))
          (local.get $builder)
          (local.get $cell))
        (%let $snapshot
          (call (%inst $list-builder.finish (ref $Cell)) (local.get $builder))
          (block
            (result i32)
            (struct.set $Cell $value (local.get $cell) (i64.const 42))
            (%match
              (call (%inst $dict.get (ref $Cell)) (local.get $dict) (%text.lit "cell"))
              (case none () (i32.const 0))
              (case some ($stored)
                (i32.and
                  (ref.eq
                    (local.get $stored)
                    (%list.get (ref $Cell) (local.get $snapshot) (i32.const 0)))
                  (i64.eq (struct.get $Cell $value (local.get $stored)) (i64.const 42))))
            ))))
    ))
  (%test "dict entries survive update and clear"
    (%let $d (call (%inst $dict.new i64))
      (block
        (result i32)
        (drop
          (call (%inst $dict.set i64) (local.get $d) (%text.lit "a") (i64.const 40)))
        (drop (call (%inst $dict.set i64) (local.get $d) (%text.lit "b") (i64.const 2)))
        (%let $snapshot (call (%inst $dict.entries i64) (local.get $d))
          (block
            (result i32)
            (drop
              (call (%inst $dict.set i64) (local.get $d) (%text.lit "a") (i64.const -1)))
            (call (%inst $dict.clear i64) (local.get $d))
            (i32.and
              (i32.eq
                (%list.len (%inst $pair $text i64) (local.get $snapshot))
                (i32.const 2))
              (i64.eq
                (%loop $sum
                  (bind $i i32 (i32.const 0))
                  (bind $sum i64 (i64.const 0))
                  (result i64)
                  (if
                    (result i64)
                    (i32.eq
                      (local.get $i)
                      (%list.len (%inst $pair $text i64) (local.get $snapshot)))
                    (then (local.get $sum))
                    (else
                      (%let ($key $value)
                        (%list.get (%inst $pair $text i64) (local.get $snapshot) (local.get $i))
                        (%recur $sum
                          (i32.add (local.get $i) (i32.const 1))
                          (i64.add (local.get $sum) (local.get $value))))
                    )))
                (i64.const 42))))
        ))))
)
