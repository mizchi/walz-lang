(module (%use "./model-macros.wlt" (func $main))
  (%bench "macro JSON validation" (result i32) (%bytes.len (%string.bytes (call $main)))))
