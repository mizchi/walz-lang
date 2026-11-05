(module
  (%namespace $queue
    (%public (opaque $Heap) (func $new) (func $push) (func $pop))
    ;; Non-negative job indices. Push requires available capacity; empty pop
    ;; returns -1. The planner enqueues each job exactly once.
    (type $Ints (array (mut i32)))
    (type $Heap (struct (field $values (ref $Ints)) (field $size (mut i32))))
    (func $new (param $capacity i32) (result (ref $Heap)) (%effects allocates)
      (struct.new $Heap
        (array.new $Ints (i32.const 0) (local.get $capacity))
        (i32.const 0)))

    (func $push (param $q (ref $Heap)) (param $value i32) (%effects reads writes)
      (%let $a (struct.get $Heap $values (local.get $q))
        (%let $size (struct.get $Heap $size (local.get $q))
          (block
            (struct.set $Heap $size
              (local.get $q)
              (i32.add (local.get $size) (i32.const 1)))
            (%loop $up
              (bind $i i32 (local.get $size))
              (if
                (i32.eqz (local.get $i))
                (then
                  (array.set $Ints (local.get $a) (local.get $i) (local.get $value)))
                (else
                  (%let $parent
                    (i32.shr_u (i32.sub (local.get $i) (i32.const 1)) (i32.const 1))
                    (%let $above (array.get $Ints (local.get $a) (local.get $parent))
                      (if
                        (i32.le_u (local.get $above) (local.get $value))
                        (then
                          (array.set $Ints
                            (local.get $a)
                            (local.get $i)
                            (local.get $value)))
                        (else
                          (block
                            (array.set $Ints
                              (local.get $a)
                              (local.get $i)
                              (local.get $above))
                            (%recur $up (local.get $parent))))
                      ))))
              ))))
      ))

    (func $pop (param $q (ref $Heap)) (result i32) (%effects reads writes)
      (%let $size (struct.get $Heap $size (local.get $q))
        (if
          (result i32)
          (i32.eqz (local.get $size))
          (then (i32.const -1))
          (else
            (%let $a (struct.get $Heap $values (local.get $q))
              (%let $first (array.get $Ints (local.get $a) (i32.const 0))
                (%let $n (i32.sub (local.get $size) (i32.const 1))
                  (%let $last (array.get $Ints (local.get $a) (local.get $n))
                    (block
                      (result i32)
                      (struct.set $Heap $size (local.get $q) (local.get $n))
                      (if
                        (local.get $n)
                        (then
                          (%loop $down
                            (bind $i i32 (i32.const 0))
                            (%let $left
                              (i32.add (i32.mul (local.get $i) (i32.const 2)) (i32.const 1))
                              (if
                                (i32.ge_u (local.get $left) (local.get $n))
                                (then
                                  (array.set $Ints
                                    (local.get $a)
                                    (local.get $i)
                                    (local.get $last)))
                                (else
                                  (%let $right (i32.add (local.get $left) (i32.const 1))
                                    (%let $child
                                      (if (result i32) (i32.lt_u (local.get $right) (local.get $n)) (then (if (result i32) (i32.lt_u (array.get $Ints (local.get $a) (local.get $right)) (array.get $Ints (local.get $a) (local.get $left))) (then (local.get $right)) (else (local.get $left)))) (else (local.get $left)))
                                      (%let $value
                                        (array.get $Ints (local.get $a) (local.get $child))
                                        (if
                                          (i32.le_u
                                            (local.get $last)
                                            (local.get $value))
                                          (then
                                            (array.set $Ints
                                              (local.get $a)
                                              (local.get $i)
                                              (local.get $last)))
                                          (else
                                            (block
                                              (array.set $Ints
                                                (local.get $a)
                                                (local.get $i)
                                                (local.get $value))
                                              (%recur $down (local.get $child))))
                                        ))))
                                ))))
                        )
                        (else (nop)))
                      (local.get $first))))
              ))))
      ))))
