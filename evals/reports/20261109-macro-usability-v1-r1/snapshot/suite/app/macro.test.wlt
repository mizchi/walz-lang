(module
  (%use "./model-macros.wlt" (func $main))
  (%use "wlt:core" (func $bytes.equal))
  (%test "macro JSON application"
    (call $bytes.equal (%string.bytes (call $main))
      (%string.bytes (%string.lit "{\"accepted\":2,\"rejected\":1,\"totals\":{\"food\":42}}")))))
