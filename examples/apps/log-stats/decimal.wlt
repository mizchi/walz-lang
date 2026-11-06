(module
  (%namespace $decimal
    (%import "wlt:std"
      (type $text))
    (%import "../../../stdlib/text.wlt" (func $string.u64))
    (%public (func $text))
    (func $text (param $n i64) (result $text) (%effect reads writes allocates)
      (call $string.u64 (local.get $n))))
)
