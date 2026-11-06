(module
  (%use "wlt:core" (type $string))
  (%use "../../../stdlib/text.wlt" (func $string.u64))
  (%export (func $text))
  (func $text
    (param $n i64)
    (result $string)
    (%effect reads writes allocates)
    (call $string.u64 (local.get $n))))
