(module
  (%use "wlt:core" (type $result))
  (func $main (export "main") (result (%inst $result i64 i32)) (%effect pure)
    (%result.ok 42)))
