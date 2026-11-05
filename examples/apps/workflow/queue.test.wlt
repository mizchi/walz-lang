(module
  (%include "queue.wlt")
  (%test "empty and singleton queues"
    (%let $q (call $queue.new (i32.const 1))
      (%let $empty (i32.eq (call $queue.pop (local.get $q)) (i32.const -1))
        (block
          (result i32)
          (call $queue.push (local.get $q) (i32.const 42))
          (i32.and
            (local.get $empty)
            (i32.eq (call $queue.pop (local.get $q)) (i32.const 42))))
      )))
  (%test "reverse insertion and both heap children"
    (%let $q (call $queue.new (i32.const 100))
      (block
        (result i32)
        (%for $insert
          (bind $i i32 (i32.const 100))
          (while (local.get $i))
          (step (i32.sub (local.get $i) (i32.const 1)))
          (call $queue.push (local.get $q) (i32.sub (local.get $i) (i32.const 1))))
        (%loop $check
          (bind $i i32 (i32.const 0))
          (result i32)
          (if
            (result i32)
            (i32.eq (local.get $i) (i32.const 100))
            (then (i32.eq (call $queue.pop (local.get $q)) (i32.const -1)))
            (else
              (if
                (result i32)
                (i32.eq (call $queue.pop (local.get $q)) (local.get $i))
                (then (%recur $check (i32.add (local.get $i) (i32.const 1))))
                (else (i32.const 0))))
          ))))
  )
  (%test "new smaller priorities and duplicate values"
    (%let $q (call $queue.new (i32.const 4))
      (block
        (result i32)
        (call $queue.push (local.get $q) (i32.const 3))
        (call $queue.push (local.get $q) (i32.const 2))
        (%let $first (i32.eq (call $queue.pop (local.get $q)) (i32.const 2))
          (block
            (result i32)
            (call $queue.push (local.get $q) (i32.const 1))
            (call $queue.push (local.get $q) (i32.const 1))
            (i32.and
              (local.get $first)
              (i32.and
                (i32.eq (call $queue.pop (local.get $q)) (i32.const 1))
                (i32.and
                  (i32.eq (call $queue.pop (local.get $q)) (i32.const 1))
                  (i32.eq (call $queue.pop (local.get $q)) (i32.const 3))))
            ))))
    )))
