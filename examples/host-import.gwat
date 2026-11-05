(module
  ;; Valid output module; running it needs an embedding that supplies host.print.
  (import "host" "print" (func $print (param $value i64)))
  (func $main (export "main") (result i64)
    (block (result i64)
      (call $print (i64.const 42))
      (i64.const 42))))
