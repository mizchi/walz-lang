(module
  (%import "wlt:std"
    (type $list)
    (type $text)
    (type $option)
    (type $pair)
    (func $dict.new)
    (func $dict.set)
    (func $dict.get)
    (func $dict.remove)
    (func $dict.clear)
    (func $dict.entries)
    (func $list-builder.new)
    (func $list-builder.push)
    (func $list-builder.extend)
    (func $list-builder.finish)
    (func $list-builder.clear))
  (func $mutations
    (export "mutations")
    (param $keys (%inst $list $text))
    (param $values (%inst $list i64))
    (param $remove (%inst $list $text))
    (param $erase-all i32)
    (result (%inst $list (%inst $option i64)))
    (%effect reads writes allocates)
    (%let
      (($d (call (%inst $dict.new i64)))
       ($alias (local.get $d))
       ($out (call (%inst $list-builder.new (%inst $option i64))))
      )
      (block
        (result (%inst $list (%inst $option i64)))
        (if
          (i32.ne
            (%list.len $text (local.get $keys))
            (%list.len i64 (local.get $values)))
          (then (unreachable))
          (else (nop)))
        (%for $set
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%list.len $text (local.get $keys))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (drop
            (call
              (%inst $dict.set i64)
              (local.get $d)
              (%list.get $text (local.get $keys) (local.get $i))
              (%list.get i64 (local.get $values) (local.get $i))))
        )
        (%for $remove
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%list.len $text (local.get $remove))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (drop
            (call
              (%inst $dict.remove i64)
              (local.get $d)
              (%list.get $text (local.get $remove) (local.get $i))))
        )
        (if
          (local.get $erase-all)
          (then (call (%inst $dict.clear i64) (local.get $d)))
          (else (nop)))
        (%for $collect
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%list.len $text (local.get $keys))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (call
            (%inst $list-builder.push (%inst $option i64))
            (local.get $out)
            (call
              (%inst $dict.get i64)
              (local.get $alias)
              (%list.get $text (local.get $keys) (local.get $i))))
        )
        (call (%inst $list-builder.finish (%inst $option i64)) (local.get $out))))
  )

  (func $snapshots
    (export "snapshots")
    (param $input (%inst $list $text))
    (result (%inst $pair (%inst $list $text) (%inst $list $text)))
    (%effect reads writes allocates)
    (%let $b (call (%inst $list-builder.new $text))
      (block
        (result (%inst $pair (%inst $list $text) (%inst $list $text)))
        (call (%inst $list-builder.extend $text) (local.get $b) (local.get $input))
        (%let $snapshot (call (%inst $list-builder.finish $text) (local.get $b))
          (block
            (result (%inst $pair (%inst $list $text) (%inst $list $text)))
            (call
              (%inst $list-builder.push $text)
              (local.get $b)
              (%text.lit "discarded"))
            (call (%inst $list-builder.clear $text) (local.get $b))
            (call (%inst $list-builder.push $text) (local.get $b) (%text.lit "later"))
            (%pair.new
              (local.get $snapshot)
              (call (%inst $list-builder.finish $text) (local.get $b))))
        ))))

  (func $entries
    (export "entries")
    (param $keys (%inst $list $text))
    (param $values (%inst $list i64))
    (result (%inst $list (%inst $pair $text i64)))
    (%effect reads writes allocates)
    (%let $d (call (%inst $dict.new i64))
      (block
        (result (%inst $list (%inst $pair $text i64)))
        (%for $fill
          (bind $i i32 (i32.const 0))
          (while (i32.lt_u (local.get $i) (%list.len $text (local.get $keys))))
          (step (i32.add (local.get $i) (i32.const 1)))
          (drop
            (call
              (%inst $dict.set i64)
              (local.get $d)
              (%list.get $text (local.get $keys) (local.get $i))
              (%list.get i64 (local.get $values) (local.get $i))))
        )
        (%let $snapshot (call (%inst $dict.entries i64) (local.get $d))
          (block
            (result (%inst $list (%inst $pair $text i64)))
            (if
              (%list.len $text (local.get $keys))
              (then
                (drop
                  (call
                    (%inst $dict.set i64)
                    (local.get $d)
                    (%list.get $text (local.get $keys) (i32.const 0))
                    (i64.const -1))))
              (else (nop)))
            (call (%inst $dict.clear i64) (local.get $d))
            (local.get $snapshot))))
    )))
