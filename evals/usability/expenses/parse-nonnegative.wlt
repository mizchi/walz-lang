(module
  (%use "./parse.wlt" (type $Row) (type $Error) (func $parse $original))
  (%use "wlt:core" (type $string) (type $result))
  (%export (type $Row) (type $Error) (func $parse))
  (func $parse
    (param $line $string)
    (result (%inst $result (ref $Row) (ref $Error)))
    (%effect reads writes allocates)
    (%result.do
      ($row (call $original $line))
      (if
        (result (%inst $result (ref $Row) (ref $Error)))
        (< (struct.get $Row $amount $row) 0)
        (then (%result.err (%variant.new $Error invalid-amount)))
        (else (%result.ok $row))))
  ))
