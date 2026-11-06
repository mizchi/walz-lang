(module
  (%namespace $report
    (%import "wlt:std"
      (type $future)
      (type $result)
      (type $text)
      (type $unit))
    (%import "../../../stdlib/json.wlt"
      (type $json.Object)
      (func $json.finish)
      (func $json.new)
      (func $json.object)
      (func $json.u64))
    (import "wlt:host/text-io@0.1.0" "write-line"
      (func $write-line (param $line $text) (result (%inst $future (%inst $result $unit i32))))
    )
    (%public (func $write))
    (func $bucket (param $s (ref $stats.State)) (param $level (ref $parse.Level))
      (result (ref $json.Object)) (%effects reads writes allocates)
      (%let $o (call $json.new)
        (block
          (result (ref $json.Object))
          (call $json.u64
            (local.get $o)
            (%text.lit "count")
            (call $stats.count (local.get $s) (local.get $level)))
          (call $json.u64
            (local.get $o)
            (%text.lit "total_ms")
            (call $stats.total (local.get $s) (local.get $level)))
          (call $json.u64
            (local.get $o)
            (%text.lit "max_ms")
            (call $stats.maximum (local.get $s) (local.get $level)))
          (local.get $o))))

    (func $write (param $s (ref $stats.State)) (result (%inst $result $unit i32))
      (%effects reads writes allocates host suspends)
      (%let $o (call $json.new)
        (%let $levels (call $json.new)
          (block
            (result (%inst $result $unit i32))
            (call $json.u64
              (local.get $o)
              (%text.lit "accepted")
              (call $stats.accepted (local.get $s)))
            (call $json.u64
              (local.get $o)
              (%text.lit "rejected")
              (call $stats.rejected (local.get $s)))
            (call $json.u64
              (local.get $o)
              (%text.lit "ignored")
              (call $stats.ignored (local.get $s)))
            (call $json.object
              (local.get $levels)
              (%text.lit "INFO")
              (call $bucket (local.get $s) (%variant.new $parse.Level info)))
            (call $json.object
              (local.get $levels)
              (%text.lit "WARN")
              (call $bucket (local.get $s) (%variant.new $parse.Level warn)))
            (call $json.object
              (local.get $levels)
              (%text.lit "ERROR")
              (call $bucket (local.get $s) (%variant.new $parse.Level error)))
            (call $json.object (local.get $o) (%text.lit "levels") (local.get $levels))
            (%future.await
              (%inst $result $unit i32)
              (call $write-line (call $json.finish (local.get $o))))
          ))))
  ))
