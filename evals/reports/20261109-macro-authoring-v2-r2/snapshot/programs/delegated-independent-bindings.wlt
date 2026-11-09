(module (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name ?value) ...) ?body)))
  (%macro $outer (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body) (%$bindings ((?name ?value) ...) ?body)))
  (func $main (export "main") (result i32) (%effect pure)
    (%$outer (($a (i32.const 20)) ($b (i32.const 22))) (+ $a $b))))
