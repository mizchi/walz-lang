(module
  (include {{PROGRAM}})
  (test "case-0" (i32.eq (call $read (struct.new $Cell (i32.const -4))) (i32.const -4)))
  (test "case-1" (i32.eq (call $read (struct.new $Cell (i32.const 0))) (i32.const 0)))
  (test "case-2" (i32.eq (call $read (struct.new $Cell (i32.const 42))) (i32.const 42)))
)
