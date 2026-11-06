(module
  (%use "./domain.wlt" (type $Error $domain.Error))
  (%use "./graph.wlt"
    (func $at $graph.at)
    (func $id $graph.id)
    (type $Graph $graph.Graph))
  (%use "./planner.wlt"
    (func $count $planner.count)
    (func $finish $planner.finish)
    (func $makespan $planner.makespan)
    (func $start $planner.start)
    (func $worker $planner.worker)
    (type $Run $planner.Run))
  (%use "wlt:core" (type $future) (type $result) (type $string) (type $unit))
  (%use "../../../stdlib/json.wlt"
    (type $json-object)
    (func $json.object-builder.boolean)
    (func $json.object-builder.finish)
    (func $json.object-builder.new)
    (func $json.object-builder.string)
    (func $json.object-builder.u64))
  (%export (func $failure) (func $plan))
  (import "wlt:host/text-io@0.1.0" "write-line"
    (func $write
      (param $line $string)
      (result (%inst $future (%inst $result $unit i32))))
  )
  (func $base
    (param $n i32)
    (result (ref $json-object))
    (%effect reads writes allocates)
    (%let $o (call $json.object-builder.new)
      (block
        (result (ref $json-object))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "line")
          (i64.extend_i32_u (local.get $n)))
        (local.get $o))))

  (func $error
    (param $n i32)
    (param $name $string)
    (param $detail $string)
    (result $string)
    (%effect reads writes allocates)
    (%let $o (call $base (local.get $n))
      (block
        (result $string)
        (call $json.object-builder.boolean
          (local.get $o)
          (%string.lit "ok")
          (i32.const 0))
        (call $json.object-builder.string
          (local.get $o)
          (%string.lit "error")
          (local.get $name))
        (call $json.object-builder.string
          (local.get $o)
          (%string.lit "detail")
          (local.get $detail))
        (call $json.object-builder.finish (local.get $o))))
  )

  (func $failure
    (param $n i32)
    (param $e (ref $domain.Error))
    (result (%inst $result $unit i32))
    (%effect reads writes allocates host suspends)
    (%future.await
      (%inst $result $unit i32)
      (call $write
        (%match $domain.Error (result $string)
          (local.get $e)
          (case invalid ($detail)
            (call $error
              (local.get $n)
              (%string.lit "invalid-command")
              (local.get $detail)))
          (case duplicate ($detail)
            (call $error
              (local.get $n)
              (%string.lit "duplicate-task")
              (local.get $detail)))
          (case unknown ($detail)
            (call $error
              (local.get $n)
              (%string.lit "unknown-task")
              (local.get $detail)))
          (case capacity ($detail)
            (call $error (local.get $n) (%string.lit "capacity") (local.get $detail)))
          (case cycle ($detail)
            (call $error (local.get $n) (%string.lit "cycle") (local.get $detail))))
      )))

  (func $header
    (param $n i32)
    (param $r (ref $planner.Run))
    (param $workers i32)
    (result $string)
    (%effect reads writes allocates)
    (%let $o (call $base (local.get $n))
      (block
        (result $string)
        (call $json.object-builder.boolean
          (local.get $o)
          (%string.lit "ok")
          (i32.const 1))
        (call $json.object-builder.string
          (local.get $o)
          (%string.lit "kind")
          (%string.lit "plan"))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "workers")
          (i64.extend_i32_u (local.get $workers)))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "tasks")
          (i64.extend_i32_u (call $planner.count (local.get $r))))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "makespan")
          (call $planner.makespan (local.get $r)))
        (call $json.object-builder.finish (local.get $o))))
  )

  (func $job
    (param $n i32)
    (param $g (ref $graph.Graph))
    (param $r (ref $planner.Run))
    (param $i i32)
    (result $string)
    (%effect reads writes allocates)
    (%let $o (call $base (local.get $n))
      (block
        (result $string)
        (call $json.object-builder.string
          (local.get $o)
          (%string.lit "kind")
          (%string.lit "job"))
        (call $json.object-builder.string
          (local.get $o)
          (%string.lit "id")
          (call $graph.id (call $graph.at (local.get $g) (local.get $i))))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "worker")
          (i64.extend_i32_u (call $planner.worker (local.get $r) (local.get $i))))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "start")
          (call $planner.start (local.get $r) (local.get $i)))
        (call $json.object-builder.u64
          (local.get $o)
          (%string.lit "finish")
          (call $planner.finish (local.get $r) (local.get $i)))
        (call $json.object-builder.finish (local.get $o))))
  )

  (func $plan
    (param $n i32)
    (param $g (ref $graph.Graph))
    (param $r (ref $planner.Run))
    (param $workers i32)
    (result (%inst $result $unit i32))
    (%effect reads writes allocates host suspends)
    (%result.let () (%inst $result $unit i32) $unit
      (%future.await
        (%inst $result $unit i32)
        (call $write (call $header (local.get $n) (local.get $r) (local.get $workers))))

      (%loop $emit
        (bind $i i32 (i32.const 0))
        (result (%inst $result $unit i32))
        (if
          (result (%inst $result $unit i32))
          (i32.eq (local.get $i) (call $planner.count (local.get $r)))
          (then (%result.ok))
          (else
            (%result.let () (%inst $result $unit i32) $unit
              (%future.await
                (%inst $result $unit i32)
                (call $write
                  (call $job
                    (local.get $n)
                    (local.get $g)
                    (local.get $r)
                    (local.get $i))))

              (%recur $emit (i32.add (local.get $i) (i32.const 1))))
          ))))
  ))
