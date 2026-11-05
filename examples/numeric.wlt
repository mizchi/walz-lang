(module
  ;; Newton iteration, illustrating the workaround for the missing f64.sqrt op.
  ;; Thirty-two iterations are sufficient for these examples, not all f64 inputs.
  (func $sqrt (export "sqrt") (param $x f64) (result f64)
    (if (result f64) (f64.lt (local.get $x) (f64.const 0))
      (then (unreachable))
      (else
        (if (result f64) (f64.eq (local.get $x) (f64.const 0))
          (then (f64.const 0))
          (else
            (%loop $newton (bind $i i32 (i32.const 0)) (bind $estimate f64 (local.get $x))
              (result f64)
              (if (result f64) (i32.lt_u (local.get $i) (i32.const 32))
                (then (%recur $newton (i32.add (local.get $i) (i32.const 1))
                  (f64.div (f64.add (local.get $estimate)
                    (f64.div (local.get $x) (local.get $estimate))) (f64.const 2))))
                (else (local.get $estimate)))))))))
  (func $main (export "main") (result f64)
    (call $sqrt (f64.const 1764))))
