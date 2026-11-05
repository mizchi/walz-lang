(module
  (include "../../../stdlib/text.wlt")
  (namespace $decimal
    (public (func $text))
    (func $text
      (param $n i64)
      (result text)
      (effects reads writes allocates)
      (call $string.u64 (local.get $n))
    )
  )
)
