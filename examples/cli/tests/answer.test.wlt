(module
  (%use "../math.wlt" (func $answer))

  (%test "answer"
    (i32.eq (call $answer) 42)))
