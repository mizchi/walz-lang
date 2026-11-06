(module
  (%import "wlt:std"
    (type $text)
    (type $result)
    (func $json-value.parse)
    (func $json-value.stringify))
  (func $input-length
    (export "input-length")
    (param $source $text)
    (result i32)
    (%effect pure)
    (%text.byte-len (local.get $source)))

  (func $parse-status
    (export "parse-status")
    (param $source $text)
    (result i32)
    (%effect allocates reads writes)
    (%match (call $json-value.parse (local.get $source))
      (case ok ($value) (i32.const 1))
      (case err ($offset) (i32.const 0))))

  (func $normalize
    (export "normalize")
    (param $source $text)
    (result (%inst $result $text i32))
    (%effect allocates reads writes)
    (%result.do
      ($value (call $json-value.parse (local.get $source)))
      (%result.ok (call $json-value.stringify (local.get $value))))
  ))
