(module (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name ?value) ...) ?body)))
  (func $main (export "main") (result i32) (%effect pure)
    (%let $a (type i32) 40
      (%$bindings (($a (i32.const 1)) ($b $a)) (+ $a $b)))))
