(module
  (%use "./math.wlt" (func $answer))

  (%bench "answer" (result i32)
    (call $answer)))
