(module
  (%use "./model.wlt" (func $main))
  (%bench "expense aggregation" (result i32) (%bytes.len (%string.bytes (call $main))))
)
