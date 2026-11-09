(module
  (%macro $inner (rule (?name:ident ?value:expr ?body:expr)
    (bind ?name ?body) (%let ?name ?value ?body)))
  (%macro $outer (rule (?name:ident ?value:expr ?body:expr)
    (bind ?name ?body) (%$inner ?name ?value (block (result i32) ?body))))
  (func $main (export "main") (result i32) (%effect pure)
    (%$outer $answer (i32.const 42) $answer)))
