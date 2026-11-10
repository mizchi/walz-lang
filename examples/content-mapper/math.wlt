(module
  (func $add (export "add-one") (param $n i64) (result i64)
    (%effect pure) (+ $n 1)))
