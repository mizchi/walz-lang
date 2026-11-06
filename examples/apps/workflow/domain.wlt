(module
  (%use "wlt:core" (type $string))
  (%export (type $Command) (type $Error))
  (type $Command
    (%variant (skip) (invalid) (task $string i64) (after $string $string) (plan i32)))
  (type $Error
    (%variant
      (invalid $string)
      (duplicate $string)
      (unknown $string)
      (capacity $string)
      (cycle $string))))
