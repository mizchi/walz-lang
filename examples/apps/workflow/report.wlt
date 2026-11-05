(module
  (namespace $report
    (public (func $failure) (func $plan))
    (import "wlt:host/text-io@0.1.0" "write-line"
      (func $write (param $line text) (result (future (result unit i32))))
    )
    (func $number (param $n i32) (result text) (effects reads writes allocates)
      (call $string.u64 (i64.extend_i32_u (local.get $n))))

    (func $line (param $n i32) (result text) (effects reads writes allocates)
      (call $string.concat (text.lit "{\"line\":") (call $number (local.get $n))))

    (func $error (param $name text) (param $detail text) (result text)
      (effects reads writes allocates)
      (call $string.concat
        (call $string.concat
          (text.lit ",\"ok\":false,\"error\":")
          (call $string.json (local.get $name)))
        (call $string.concat
          (text.lit ",\"detail\":")
          (call $string.concat (call $string.json (local.get $detail)) (text.lit "}"))))
    )

    (func $failure (param $n i32) (param $e (ref $domain.Error))
      (result (result unit i32)) (effects reads writes allocates host suspends)
      (let $body
        (match $domain.Error (result text)
          (local.get $e)
          (case invalid ($detail)
            (call $error (text.lit "invalid-command") (local.get $detail)))
          (case duplicate ($detail)
            (call $error (text.lit "duplicate-task") (local.get $detail)))
          (case unknown ($detail)
            (call $error (text.lit "unknown-task") (local.get $detail)))
          (case capacity ($detail)
            (call $error (text.lit "capacity") (local.get $detail)))
          (case cycle ($detail) (call $error (text.lit "cycle") (local.get $detail))))
        (future.await
          (result unit i32)
          (call $write
            (call $string.concat (call $line (local.get $n)) (local.get $body))))
      ))

    (func $header (param $n i32) (param $r (ref $planner.Run)) (param $workers i32)
      (result text) (effects reads writes allocates)
      (let $a
        (call $string.concat (call $line (local.get $n)) (text.lit ",\"ok\":true,\"kind\":\"plan\",\"workers\":"))
        (let $b (call $string.concat (local.get $a) (call $number (local.get $workers)))
          (let $c
            (call $string.concat
              (local.get $b)
              (call $string.concat
                (text.lit ",\"tasks\":")
                (call $number (call $planner.count (local.get $r))))
            )
            (call $string.concat
              (local.get $c)
              (call $string.concat
                (text.lit ",\"makespan\":")
                (call $string.concat
                  (call $string.u64 (call $planner.makespan (local.get $r)))
                  (text.lit "}"))))
          ))))

    (func $job (param $n i32) (param $g (ref $graph.Graph))
      (param $r (ref $planner.Run)) (param $i i32) (result text)
      (effects reads writes allocates)
      (let $a
        (call $string.concat (call $line (local.get $n)) (text.lit ",\"kind\":\"job\",\"id\":"))
        (let $b
          (call $string.concat
            (local.get $a)
            (call $string.json
              (call $graph.id (call $graph.at (local.get $g) (local.get $i))))
          )
          (let $c
            (call $string.concat
              (local.get $b)
              (call $string.concat
                (text.lit ",\"worker\":")
                (call $number (call $planner.worker (local.get $r) (local.get $i))))
            )
            (let $d
              (call $string.concat
                (local.get $c)
                (call $string.concat
                  (text.lit ",\"start\":")
                  (call $string.u64 (call $planner.start (local.get $r) (local.get $i))))
              )
              (call $string.concat
                (local.get $d)
                (call $string.concat
                  (text.lit ",\"finish\":")
                  (call $string.concat
                    (call $string.u64
                      (call $planner.finish (local.get $r) (local.get $i)))
                    (text.lit "}"))))
            ))))
    )

    (func $plan (param $n i32) (param $g (ref $graph.Graph))
      (param $r (ref $planner.Run)) (param $workers i32) (result (result unit i32))
      (effects reads writes allocates host suspends)
      (result.let () (result unit i32) unit
        (future.await
          (result unit i32)
          (call $write
            (call $header (local.get $n) (local.get $r) (local.get $workers))))
        (loop $emit
          (bind $i i32 (i32.const 0))
          (result (result unit i32))
          (if
            (result (result unit i32))
            (i32.eq (local.get $i) (call $planner.count (local.get $r)))
            (then (result.ok))
            (else
              (result.let () (result unit i32) unit
                (future.await
                  (result unit i32)
                  (call $write
                    (call $job
                      (local.get $n)
                      (local.get $g)
                      (local.get $r)
                      (local.get $i))))
                (recur $emit (i32.add (local.get $i) (i32.const 1))))
            ))))
    )))
