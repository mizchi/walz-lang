(module
  (type $Choice (variant (none) (some i32)))
  (func $read (param $choice (ref $Choice)) (result i32)
    (match $Choice (result i32) (local.get $choice)
      (case none () (i32.const 0))
      (case some ($value) (local.get $value)))))
