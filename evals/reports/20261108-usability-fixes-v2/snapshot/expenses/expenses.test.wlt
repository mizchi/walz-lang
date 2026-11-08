(module
  (%use "./model.wlt" (func $main))
  (%use "wlt:core" (func $bytes.equal))
  (%test "public entry runs"
    (call $bytes.equal
      (%string.bytes (call $main))
      (%string.bytes
        (%string.lit "{\"accepted\":2,\"rejected\":1,\"totals\":{\"food\":42}}"))))
)
