(module
  (%namespace $report
    (%import "../../../stdlib/json.wlt"
      (type $json.Object)
      (func $json.boolean)
      (func $json.finish)
      (func $json.new)
      (func $json.string)
      (func $json.u64))
    (%public (func $failure) (func $plan))
    (import "wlt:host/text-io@0.1.0" "write-line"
      (func $write (param $line %text) (result (%future (%result %unit i32))))
    )
    (func $base (param $n i32) (result (ref $json.Object))
      (%effects reads writes allocates)
      (%let $o (call $json.new)
        (block
          (result (ref $json.Object))
          (call $json.u64
            (local.get $o)
            (%text.lit "line")
            (i64.extend_i32_u (local.get $n)))
          (local.get $o))))

    (func $error (param $n i32) (param $name %text) (param $detail %text) (result %text)
      (%effects reads writes allocates)
      (%let $o (call $base (local.get $n))
        (block
          (result %text)
          (call $json.boolean (local.get $o) (%text.lit "ok") (i32.const 0))
          (call $json.string (local.get $o) (%text.lit "error") (local.get $name))
          (call $json.string (local.get $o) (%text.lit "detail") (local.get $detail))
          (call $json.finish (local.get $o))))
    )

    (func $failure (param $n i32) (param $e (ref $domain.Error))
      (result (%result %unit i32)) (%effects reads writes allocates host suspends)
      (%future.await
        (%result %unit i32)
        (call $write
          (%match $domain.Error (result %text)
            (local.get $e)
            (case invalid ($detail)
              (call $error
                (local.get $n)
                (%text.lit "invalid-command")
                (local.get $detail)))
            (case duplicate ($detail)
              (call $error
                (local.get $n)
                (%text.lit "duplicate-task")
                (local.get $detail)))
            (case unknown ($detail)
              (call $error (local.get $n) (%text.lit "unknown-task") (local.get $detail)))
            (case capacity ($detail)
              (call $error (local.get $n) (%text.lit "capacity") (local.get $detail)))
            (case cycle ($detail)
              (call $error (local.get $n) (%text.lit "cycle") (local.get $detail))))
        )))

    (func $header (param $n i32) (param $r (ref $planner.Run)) (param $workers i32)
      (result %text) (%effects reads writes allocates)
      (%let $o (call $base (local.get $n))
        (block
          (result %text)
          (call $json.boolean (local.get $o) (%text.lit "ok") (i32.const 1))
          (call $json.string (local.get $o) (%text.lit "kind") (%text.lit "plan"))
          (call $json.u64
            (local.get $o)
            (%text.lit "workers")
            (i64.extend_i32_u (local.get $workers)))
          (call $json.u64
            (local.get $o)
            (%text.lit "tasks")
            (i64.extend_i32_u (call $planner.count (local.get $r))))
          (call $json.u64
            (local.get $o)
            (%text.lit "makespan")
            (call $planner.makespan (local.get $r)))
          (call $json.finish (local.get $o))))
    )

    (func $job (param $n i32) (param $g (ref $graph.Graph))
      (param $r (ref $planner.Run)) (param $i i32) (result %text)
      (%effects reads writes allocates)
      (%let $o (call $base (local.get $n))
        (block
          (result %text)
          (call $json.string (local.get $o) (%text.lit "kind") (%text.lit "job"))
          (call $json.string
            (local.get $o)
            (%text.lit "id")
            (call $graph.id (call $graph.at (local.get $g) (local.get $i))))
          (call $json.u64
            (local.get $o)
            (%text.lit "worker")
            (i64.extend_i32_u (call $planner.worker (local.get $r) (local.get $i))))
          (call $json.u64
            (local.get $o)
            (%text.lit "start")
            (call $planner.start (local.get $r) (local.get $i)))
          (call $json.u64
            (local.get $o)
            (%text.lit "finish")
            (call $planner.finish (local.get $r) (local.get $i)))
          (call $json.finish (local.get $o))))
    )

    (func $plan (param $n i32) (param $g (ref $graph.Graph))
      (param $r (ref $planner.Run)) (param $workers i32) (result (%result %unit i32))
      (%effects reads writes allocates host suspends)
      (%result.let () (%result %unit i32) %unit
        (%future.await
          (%result %unit i32)
          (call $write
            (call $header (local.get $n) (local.get $r) (local.get $workers))))
        (%loop $emit
          (bind $i i32 (i32.const 0))
          (result (%result %unit i32))
          (if
            (result (%result %unit i32))
            (i32.eq (local.get $i) (call $planner.count (local.get $r)))
            (then (%result.ok))
            (else
              (%result.let () (%result %unit i32) %unit
                (%future.await
                  (%result %unit i32)
                  (call $write
                    (call $job
                      (local.get $n)
                      (local.get $g)
                      (local.get $r)
                      (local.get $i))))
                (%recur $emit (i32.add (local.get $i) (i32.const 1))))
            ))))
    )))
