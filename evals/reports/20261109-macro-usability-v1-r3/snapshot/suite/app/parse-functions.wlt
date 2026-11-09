(module
  (%use "./schema.wlt" (type $Row) (type $Error)
    (func $parse-json) (func $string-field) (func $s64-field))
  (%use "wlt:core" (type $string) (type $result))
  (%export (type $Row) (type $Error) (func $parse))
  (func $parse (param $line $string)
    (result (%inst $result (ref $Row) (ref $Error)))
    (%effect reads writes allocates)
    (%result.do
      ($object (call $parse-json $line))
      ($category (call $string-field $object "category"))
      ($amount (call $s64-field $object "amount"))
      (%result.ok (%$Row::{:category $category :amount $amount})))))
