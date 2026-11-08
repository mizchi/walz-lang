(module
  (%include {{PROGRAM}})
  (%test "case-0" (i32.eq (call $read (%variant.new $Choice none)) (i32.const 0)))
  (%test "case-1" (i32.eq (call $read (%variant.new $Choice some (i32.const -7))) (i32.const -7)))
  (%test "case-2" (i32.eq (call $read (%variant.new $Choice some (i32.const 42))) (i32.const 42)))
)
