(module
  (%use "wlt:core"
    (type $string)
    (type $option)
    (type $result)
    (type $json-value)
    (func $json.parse)
    (func $json.get)
    (func $json.as-string)
    (func $json.as-number)
    (func $option.ok-or)
    (func $string.parse-s64))
  (%export (type $Row) (type $Error) (func $parse))
  (type $Row (struct (field $category $string) (field $amount i64)))
  (type $Error
    (%variant
      (invalid-json i32)
      (missing-field $string)
      (wrong-type $string)
      (invalid-amount)
      (overflow)))
  (%type.alias $Failure (ref $Error))
  (func $parse-json
    (param $line $string)
    (result (%inst $result (ref $json-value) $Failure))
    (%effect reads writes allocates)
    (%match (call $json.parse $line)
      (case err ($offset) (%result.err (%variant.new $Error invalid-json $offset)))
      (case ok ($value) (%result.ok $value))))

  (func $field
    (param $object (ref $json-value))
    (param $key $string)
    (result (%inst $result (ref $json-value) $Failure))
    (%effect reads writes allocates)
    (call
      (%inst $option.ok-or (ref $json-value) $Failure)
      (call $json.get $object $key)
      (%variant.new $Error missing-field $key)))

  (func $parse
    (param $line $string)
    (result (%inst $result (ref $Row) $Failure))
    (%effect reads writes allocates)
    (%result.do
      ($object (call $parse-json $line))
      ($category-value (call $field $object "category"))
      ($category
        (call
          (%inst $option.ok-or $string $Failure)
          (call $json.as-string $category-value)
          (%variant.new $Error wrong-type "category")))
      ($amount-value (call $field $object "amount"))
      ($amount-text
        (call
          (%inst $option.ok-or $string $Failure)
          (call $json.as-number $amount-value)
          (%variant.new $Error wrong-type "amount")))
      ($amount
        (call
          (%inst $option.ok-or i64 $Failure)
          (call $string.parse-s64 $amount-text)
          (%variant.new $Error invalid-amount)))
      (%result.ok (%$Row::{:category $category :amount $amount}))))
)
