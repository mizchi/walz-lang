(module
  (%closure $Work (%capture $n i32) (result i32) (%effect pure) (+ $n 1))
  (func $main (export "main") (result i32) (%effect host reads writes allocates suspends)
    (%task.scope (result i32)
      (%let $a (%task.spawn i32 $Work (%closure.new $Work 20))
        (%let $b (%task.spawn i32 $Work (%closure.new $Work 20))
          (+ (%task.join i32 (%move $a)) (%task.join i32 (%move $b))))))))
