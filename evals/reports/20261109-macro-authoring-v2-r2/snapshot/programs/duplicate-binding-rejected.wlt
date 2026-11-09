(module (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name ?value) ...) ?body)))
  (func $main (export "main") (result i32) (%effect pure)
    (%$bindings (($a (i32.const 1)) ($a (i32.const 2))) $a)))
