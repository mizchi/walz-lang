(module
  (%macro $add-one (rule (?value:expr) (i32.add ?value 1)))
  (func $main (export "main") (result i32) (%effect pure)
    (%$add-one (i64.const 41))))
