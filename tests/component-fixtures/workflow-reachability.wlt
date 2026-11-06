(module
  (%use "wlt:core"
    (type $result)
    (type $unit))
  (%include "../../examples/apps/workflow/compute.wlt")
  (func $must (param $result (%inst $result $unit (ref $domain.Error)))
    (%match (local.get $result)
      (case ok () (nop))
      (case err ($e) (unreachable))))

  (func $new (export "new") (result (ref $graph.Graph))
    (%let $g (call $graph.new)
      (block
        (result (ref $graph.Graph))
        (call $must (call $graph.add (local.get $g) (%string.lit "Aa") (i64.const 1)))
        (call $must (call $graph.add (local.get $g) (%string.lit "BB") (i64.const 2)))
        (local.get $g))))

  (func $first (export "first") (param $g (ref $graph.Graph)) (result (ref $graph.Job))
    (call $graph.at (local.get $g) (i32.const 0)))

  (func $second (export "second") (param $g (ref $graph.Graph))
    (result (ref $graph.Job))
    (call $graph.at (local.get $g) (i32.const 1)))

  (func $duration (export "duration") (param $job (ref $graph.Job)) (result i64)
    (call $graph.duration (local.get $job))))
