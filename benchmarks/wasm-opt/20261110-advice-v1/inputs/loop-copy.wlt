(module
  (%use "wlt:core" (type $bytes))
  (type $Input (array i32))
  (func $make (export "make") (param $length i32) (result $bytes) (%effect allocates)
    (%bytes.from-array $Input (array.new $Input 21 $length)))
  (func $copy (param $input $bytes) (result $bytes) (%effect pure)
    (%bytes.slice $input 0 (%bytes.len $input)))

  ;; The copy's payload is dynamic, and is rebuilt on every loop iteration.
  (func $sum (export "sum") (param $input $bytes) (param $limit i32) (result i32) (%effect pure)
    (%loop $again (bind $i i32 0) (bind $sum i32 0) (result i32)
      (if (result i32) (= $i $limit)
        (then $sum)
        (else (%let $part (call $copy $input)
          (%recur $again (+ $i 1) (+ $sum (%bytes.get $part 0))))))))
  (func $main (export "main") (result i32) (%effect allocates)
    (call $sum (call $make 2) 2)))
