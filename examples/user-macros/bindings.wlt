(module
  ;; Captured initializers retain their caller's scope; ?body sees all names.
  (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name (type i64) ?value) ...) ?body)))

  ;; Binding interfaces can be forwarded without introducing implicit capture.
  (%macro $with-values
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%$bindings ((?name ?value) ...) ?body)))

  (func $main (export "main") (result i64) (%effect pure)
    (%$with-values (($a (i64.const 20)) ($b (i64.const 22)))
      (+ $a $b)))
  (%test "delegated repeated bindings" (= (call $main) (i64.const 42))))
