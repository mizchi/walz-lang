(module
  (%import "wlt:std"
    (type $result)
    (type $pair)
    (type $stream)
    (type $text)
    (type $unit)
    (type $option)
    (func $string.parse-u64)
    (func $builder.new)
    (func $builder.text)
    (func $builder.u64)
    (func $builder.finish))
  (%include "program.wlt")

  (%type.alias $CountState (%inst $pair (ref $stock.State) i32))
  (%type.alias $Count64State (%inst $pair (ref $stock.State) i64))

  (func $main
    (export "main")
    (result (%inst $result i32 i32))
    (%effects reads writes allocates host suspends)
    (%let
      (($stock (call $stock.new (i32.const 4096)))
       ($initial $CountState
         (%pair.new (local.get $stock) (i32.const 0)))
       ($input (call $read-lines)))
      (%result.let ($final-state)
        (%inst $result $CountState i32) i32
        (%stream.fold-result $text $CountState i32
          (%move $input)
          (local.get $initial)
          ($state $line)
          (%let ($stock $count) (local.get $state)
            (%result.let ()
              (%inst $result $unit i32) $CountState
              (call $app.process (local.get $stock) (local.get $line))
              (%result.ok
                (%pair.new
                  (local.get $stock)
                  (i32.add (local.get $count) (i32.const 1)))))))
        (%let ($stock $count) (local.get $final-state)
          (%result.ok (local.get $count))))))

  (func $count64
    (export "count64")
    (result (%inst $result i64 i32))
    (%effects reads writes allocates host suspends)
    (%let
      (($stock (call $stock.new (i32.const 4096)))
       ($initial $Count64State
         (%pair.new (local.get $stock) (i64.const 0)))
       ($input (call $read-lines)))
      (%result.let ($final-state)
        (%inst $result $Count64State i32) i64
        (%stream.fold-result $text $Count64State i32
          (%move $input)
          (local.get $initial)
          ($state $line)
          (%let ($stock $count) (local.get $state)
            (%result.let ()
              (%inst $result $unit i32) $Count64State
              (call $app.process (local.get $stock) (local.get $line))
              (%result.ok
                (%pair.new
                  (local.get $stock)
                  (i64.add (local.get $count) (i64.const 1)))))))
        (%let ($stock $count) (local.get $final-state)
          (%result.ok (local.get $count))))))

  (func $format-count
    (export "format-count")
    (param $input $text)
    (result (%inst $result $text i32))
    (%effects reads writes allocates)
    (%match (%inst $option i64) (result (%inst $result $text i32))
      (call $string.parse-u64 (local.get $input))
      (case none () (%result.err (i32.const 1)))
      (case some ($count)
        (%let $builder (call $builder.new)
          (block (result (%inst $result $text i32))
            (call $builder.text (local.get $builder) (%text.lit "count="))
            (call $builder.u64 (local.get $builder) (local.get $count))
            (%result.ok (call $builder.finish (local.get $builder)))))))))
