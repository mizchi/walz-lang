(module
  (%include {{PROGRAM}})
  (%test "case-0" (i32.eq (call $inc (i32.const 0)) (i32.const 1)))
  (%test "case-1" (i32.eq (call $inc (i32.const 41)) (i32.const 42)))
  (%test "case-2" (i32.eq (call $inc (i32.const 2147483647)) (i32.const -2147483648)))
)
