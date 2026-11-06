(module
  (%use "wlt:core" (type $string) (func $string.concat))
  (func $main (export "main") (param $left $string) (param $right $string)
    (result $string) (%effect reads writes allocates)
    (call $string.concat $left $right)))
