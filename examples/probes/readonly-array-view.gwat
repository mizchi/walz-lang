(module
  ;; A builder can fill mutable arrays, but cannot expose an immutable view.
  (type $Mutable (array (mut i64)))
  (type $Immutable (array i64))
  (func $freeze (param $xs (ref $Mutable)) (result (ref $Immutable))
    (local.get $xs)))
