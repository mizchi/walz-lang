(module (func $main (export "main") (result i32) (%effect pure) (%let $dead (+ (i32.const 1) 2) (i32.const 42))))
