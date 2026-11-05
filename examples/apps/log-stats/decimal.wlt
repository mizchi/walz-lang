(module
  (%namespace $decimal
    (%import "../../../stdlib/text.wlt" (func $string.u64))
    (%public (func $text))
    (func $text (param $n i64) (result %text) (%effects reads writes allocates)
      (call $string.u64 (local.get $n))))
)
