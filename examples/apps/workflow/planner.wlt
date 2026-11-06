(module
  (%namespace $planner
    (%import "wlt:std"
      (type $result)
      (type $text))
    (%public
      (opaque $Run)
      (func $run)
      (func $count)
      (func $makespan)
      (func $start)
      (func $finish)
      (func $worker))
    ;; A separate run never changes the graph, so PLAN can be repeated safely.
    (type $Phase (%variant (pending) (running i64 i64 i32) (done i64 i64 i32)))
    (type $Phases (array (mut (ref $Phase))))
    (type $Workers (array (mut i32)))
    (type $Run
      (struct
        (field $phases (ref $Phases))
        (field $workers (ref $Workers))
        (field $span (mut i64))))
    (func $entry (param $r (ref $Run)) (param $i i32) (result (ref $Phase))
      (%effect reads)
      (array.get $Phases (struct.get $Run $phases (local.get $r)) (local.get $i)))

    (func $count (param $r (ref $Run)) (result i32) (%effect pure)
      (array.len (struct.get $Run $phases (local.get $r))))

    (func $makespan (param $r (ref $Run)) (result i64) (%effect reads)
      (struct.get $Run $span (local.get $r)))

    (func $pending (param $p (ref $Phase)) (result i32) (%effect pure)
      (%match (local.get $p)
        (case pending () (i32.const 1))
        (case running ($s $f $w) (i32.const 0))
        (case done ($s $f $w) (i32.const 0))))

    (func $due (param $p (ref $Phase)) (result i64) (%effect pure)
      (%match (local.get $p)
        (case pending () (i64.const -1))
        (case running ($s $f $w) (local.get $f))
        (case done ($s $f $w) (i64.const -1))))

    (func $start (param $r (ref $Run)) (param $i i32) (result i64) (%effect reads)
      (%match (call $entry (local.get $r) (local.get $i))
        (case pending () (unreachable))
        (case running ($s $f $w) (unreachable))
        (case done ($s $f $w) (local.get $s))))

    (func $finish (param $r (ref $Run)) (param $i i32) (result i64) (%effect reads)
      (%match (call $entry (local.get $r) (local.get $i))
        (case pending () (unreachable))
        (case running ($s $f $w) (unreachable))
        (case done ($s $f $w) (local.get $f))))

    (func $worker (param $r (ref $Run)) (param $i i32) (result i32) (%effect reads)
      (%match (call $entry (local.get $r) (local.get $i))
        (case pending () (unreachable))
        (case running ($s $f $w) (unreachable))
        (case done ($s $f $w) (local.get $w))))
    ;; The work graph points to the result, never the other way round. Reverse
    ;; adjacency, indegrees and the heap become unreachable after planning.
    (type $Ints (array (mut i32)))
    (type $Work
      (struct
        (field $result (ref $Run))
        (field $remaining (ref $Ints))
        (field $starts (ref $Ints))
        (field $edges (ref $Ints))
        (field $ready (ref $queue.Heap))))
    (func $count-edges (param $g (ref $graph.Graph)) (param $remaining (ref $Ints))
      (param $starts (ref $Ints)) (result i32) (%effect reads writes)
      (%loop $jobs
        (bind $i i32 (i32.const 0))
        (bind $total i32 (i32.const 0))
        (result i32)
        (if
          (result i32)
          (i32.eq (local.get $i) (call $graph.size (local.get $g)))
          (then (local.get $total))
          (else
            (%let $count
              (%loop $deps
                (bind $link
                  (ref null $graph.Link)
                  (call $graph.dependencies
                    (call $graph.at (local.get $g) (local.get $i))))
                (bind $n i32 (i32.const 0))
                (result i32)
                (if
                  (result i32)
                  (ref.is_null (local.get $link))
                  (then (local.get $n))
                  (else
                    (%let $edge (ref.as_non_null (local.get $link))
                      (%let $slot
                        (i32.add (call $graph.index (call $graph.edge-job (local.get $edge))) (i32.const 1))
                        (block
                          (result i32)
                          (array.set $Ints
                            (local.get $starts)
                            (local.get $slot)
                            (i32.add
                              (array.get $Ints (local.get $starts) (local.get $slot))
                              (i32.const 1)))
                          (%recur $deps
                            (call $graph.edge-next (local.get $edge))
                            (i32.add (local.get $n) (i32.const 1))))
                      ))))
              )
              (block
                (result i32)
                (array.set $Ints
                  (local.get $remaining)
                  (local.get $i)
                  (local.get $count))
                (%recur $jobs
                  (i32.add (local.get $i) (i32.const 1))
                  (i32.add (local.get $total) (local.get $count))))
            ))))
    )

    (func $prepare (param $g (ref $graph.Graph)) (param $workers i32)
      (result (ref $Work)) (%effect reads writes allocates)
      (%let $n (call $graph.size (local.get $g))
        (%let $remaining (array.new $Ints (i32.const 0) (local.get $n))
          (%let $starts
            (array.new $Ints (i32.const 0) (i32.add (local.get $n) (i32.const 1)))
            (%let $count
              (call $count-edges (local.get $g) (local.get $remaining) (local.get $starts))
              (%let $edges (array.new $Ints (i32.const 0) (local.get $count))
                (%let $cursor (array.new $Ints (i32.const 0) (local.get $n))
                  (block
                    (result (ref $Work))
                    (%for $prefix
                      (bind $i i32 (i32.const 0))
                      (while (i32.lt_u (local.get $i) (local.get $n)))
                      (step (i32.add (local.get $i) (i32.const 1)))
                      (block
                        (array.set $Ints
                          (local.get $starts)
                          (i32.add (local.get $i) (i32.const 1))
                          (i32.add
                            (array.get $Ints (local.get $starts) (local.get $i))
                            (array.get $Ints
                              (local.get $starts)
                              (i32.add (local.get $i) (i32.const 1))))
                        )
                        (array.set $Ints
                          (local.get $cursor)
                          (local.get $i)
                          (array.get $Ints (local.get $starts) (local.get $i))))
                    )
                    (%for $links
                      (bind $i i32 (i32.const 0))
                      (while (i32.lt_u (local.get $i) (local.get $n)))
                      (step (i32.add (local.get $i) (i32.const 1)))
                      (%loop $deps
                        (bind $link
                          (ref null $graph.Link)
                          (call $graph.dependencies
                            (call $graph.at (local.get $g) (local.get $i))))
                        (if
                          (ref.is_null (local.get $link))
                          (then (nop))
                          (else
                            (%let $edge (ref.as_non_null (local.get $link))
                              (%let $parent
                                (call $graph.index (call $graph.edge-job (local.get $edge)))
                                (%let $slot
                                  (array.get $Ints (local.get $cursor) (local.get $parent))
                                  (block
                                    (array.set $Ints
                                      (local.get $edges)
                                      (local.get $slot)
                                      (local.get $i))
                                    (array.set $Ints
                                      (local.get $cursor)
                                      (local.get $parent)
                                      (i32.add (local.get $slot) (i32.const 1)))
                                    (%recur $deps
                                      (call $graph.edge-next (local.get $edge))))
                                ))))
                        )))
                    (%let $heap (call $queue.new (local.get $n))
                      (%let $work
                        (struct.new $Work (struct.new $Run (array.new $Phases (%variant.new $Phase pending) (local.get $n)) (array.new $Workers (i32.const -1) (local.get $workers)) (i64.const 0)) (local.get $remaining) (local.get $starts) (local.get $edges) (local.get $heap))
                        (block
                          (result (ref $Work))
                          (%for $initial
                            (bind $i i32 (i32.const 0))
                            (while (i32.lt_u (local.get $i) (local.get $n)))
                            (step (i32.add (local.get $i) (i32.const 1)))
                            (if
                              (i32.eqz
                                (array.get $Ints (local.get $remaining) (local.get $i)))
                              (then (call $queue.push (local.get $heap) (local.get $i)))
                              (else (nop))))
                          (local.get $work))))
                  ))))
          ))))

    (func $release (param $work (ref $Work)) (param $job i32) (%effect reads writes)
      (%let $starts (struct.get $Work $starts (local.get $work))
        (%let $remaining (struct.get $Work $remaining (local.get $work))
          (%for $children
            (bind $p i32 (array.get $Ints (local.get $starts) (local.get $job)))
            (while
              (i32.lt_u
                (local.get $p)
                (array.get $Ints
                  (local.get $starts)
                  (i32.add (local.get $job) (i32.const 1))))
            )
            (step (i32.add (local.get $p) (i32.const 1)))
            (%let $child
              (array.get $Ints (struct.get $Work $edges (local.get $work)) (local.get $p))
              (%let $left
                (i32.sub (array.get $Ints (local.get $remaining) (local.get $child)) (i32.const 1))
                (block
                  (array.set $Ints
                    (local.get $remaining)
                    (local.get $child)
                    (local.get $left))
                  (if
                    (i32.eqz (local.get $left))
                    (then
                      (call $queue.push
                        (struct.get $Work $ready (local.get $work))
                        (local.get $child)))
                    (else (nop))))
              ))))
      ))

    (func $blocked (param $r (ref $Run)) (param $g (ref $graph.Graph)) (result $text)
      (%effect reads)
      (%loop $scan
        (bind $i i32 (i32.const 0))
        (result $text)
        (if
          (result $text)
          (call $pending (call $entry (local.get $r) (local.get $i)))
          (then (call $graph.id (call $graph.at (local.get $g) (local.get $i))))
          (else (%recur $scan (i32.add (local.get $i) (i32.const 1))))
        )))

    (func $fill (param $work (ref $Work)) (param $g (ref $graph.Graph)) (param $now i64)
      (%effect reads writes allocates)
      (%let $r (struct.get $Work $result (local.get $work))
        (%let $workers (struct.get $Run $workers (local.get $r))
          (%for $assign
            (bind $w i32 (i32.const 0))
            (while (i32.lt_u (local.get $w) (array.len (local.get $workers))))
            (step (i32.add (local.get $w) (i32.const 1)))
            (if
              (i32.eq
                (array.get $Workers (local.get $workers) (local.get $w))
                (i32.const -1))
              (then
                (%let $i (call $queue.pop (struct.get $Work $ready (local.get $work)))
                  (if
                    (i32.ge_s (local.get $i) (i32.const 0))
                    (then
                      (block
                        (array.set $Workers
                          (local.get $workers)
                          (local.get $w)
                          (local.get $i))
                        (array.set $Phases
                          (struct.get $Run $phases (local.get $r))
                          (local.get $i)
                          (%variant.new $Phase running
                            (local.get $now)
                            (i64.add
                              (local.get $now)
                              (call $graph.duration
                                (call $graph.at (local.get $g) (local.get $i))))
                            (local.get $w))))
                    )
                    (else (nop))))
              )
              (else (nop))))
        )))

    (func $next-time (param $r (ref $Run)) (result i64) (%effect reads)
      (%let $workers (struct.get $Run $workers (local.get $r))
        (%loop $scan
          (bind $w i32 (i32.const 0))
          (bind $next i64 (i64.const -1))
          (result i64)
          (if
            (result i64)
            (i32.eq (local.get $w) (array.len (local.get $workers)))
            (then (local.get $next))
            (else
              (%let $i (array.get $Workers (local.get $workers) (local.get $w))
                (%let $candidate
                  (if
                    (result i64)
                    (i32.lt_s (local.get $i) (i32.const 0))
                    (then (i64.const -1))
                    (else (call $due (call $entry (local.get $r) (local.get $i))))
                  )
                  (%recur $scan
                    (i32.add (local.get $w) (i32.const 1))
                    (if
                      (result i64)
                      (i64.lt_u (local.get $candidate) (local.get $next))
                      (then (local.get $candidate))
                      (else (local.get $next))))
                ))))
        )))

    (func $complete (param $work (ref $Work)) (param $now i64) (result i32)
      (%effect reads writes allocates)
      (%let $r (struct.get $Work $result (local.get $work))
        (%let $workers (struct.get $Run $workers (local.get $r))
          (%loop $finish
            (bind $w i32 (i32.const 0))
            (bind $count i32 (i32.const 0))
            (result i32)
            (if
              (result i32)
              (i32.eq (local.get $w) (array.len (local.get $workers)))
              (then (local.get $count))
              (else
                (%let $i (array.get $Workers (local.get $workers) (local.get $w))
                  (if
                    (result i32)
                    (i32.lt_s (local.get $i) (i32.const 0))
                    (then
                      (%recur $finish
                        (i32.add (local.get $w) (i32.const 1))
                        (local.get $count)))
                    (else
                      (%match (call $entry (local.get $r) (local.get $i))
                        (case pending () (unreachable))
                        (case done ($s $f $worker) (unreachable))
                        (case running ($s $f $worker)
                          (if
                            (result i32)
                            (i64.eq (local.get $f) (local.get $now))
                            (then
                              (block
                                (result i32)
                                (array.set $Phases
                                  (struct.get $Run $phases (local.get $r))
                                  (local.get $i)
                                  (%variant.new $Phase done
                                    (local.get $s)
                                    (local.get $f)
                                    (local.get $worker)))
                                (array.set $Workers
                                  (local.get $workers)
                                  (local.get $w)
                                  (i32.const -1))
                                (call $release (local.get $work) (local.get $i))
                                (%recur $finish
                                  (i32.add (local.get $w) (i32.const 1))
                                  (i32.add (local.get $count) (i32.const 1))))
                            )
                            (else
                              (%recur $finish
                                (i32.add (local.get $w) (i32.const 1))
                                (local.get $count))))
                        ))))
                ))))
        )))

    (func $run (param $g (ref $graph.Graph)) (param $workers i32)
      (result (%inst $result (ref $Run) (ref $domain.Error))) (%effect reads writes allocates)
      (if
        (result (%inst $result (ref $Run) (ref $domain.Error)))
        (i32.or
          (i32.eqz (local.get $workers))
          (i32.gt_u (local.get $workers) (i32.const 32)))
        (then (%result.err (%variant.new $domain.Error invalid (%text.lit "format"))))
        (else
          (%let $work (call $prepare (local.get $g) (local.get $workers))
            (%let $run (struct.get $Work $result (local.get $work))
              (%loop $events
                (bind $now i64 (i64.const 0))
                (bind $done i32 (i32.const 0))
                (result (%inst $result (ref $Run) (ref $domain.Error)))
                (if
                  (result (%inst $result (ref $Run) (ref $domain.Error)))
                  (i32.eq (local.get $done) (call $graph.size (local.get $g)))
                  (then
                    (block
                      (result (%inst $result (ref $Run) (ref $domain.Error)))
                      (struct.set $Run $span (local.get $run) (local.get $now))
                      (%result.ok (local.get $run))))
                  (else
                    (block
                      (result (%inst $result (ref $Run) (ref $domain.Error)))
                      (call $fill (local.get $work) (local.get $g) (local.get $now))
                      (%let $next (call $next-time (local.get $run))
                        (if
                          (result (%inst $result (ref $Run) (ref $domain.Error)))
                          (i64.eq (local.get $next) (i64.const -1))
                          (then
                            (%result.err
                              (%variant.new $domain.Error cycle
                                (call $blocked (local.get $run) (local.get $g))))
                          )
                          (else
                            (%let $count
                              (call $complete (local.get $work) (local.get $next))
                              (%recur $events
                                (local.get $next)
                                (i32.add (local.get $done) (local.get $count))))
                          ))))
                  ))))
          ))))
  ))
