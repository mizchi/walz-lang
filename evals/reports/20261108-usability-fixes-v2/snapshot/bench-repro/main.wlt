(module
  (func $main (export "main") (result i32) (%effect pure) 42)
  (%bench "main" (result i32) (call $main)))
