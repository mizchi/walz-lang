(module
  (%import "wlt:std"
    (type $text)
    (type $result)
    (type $json-value.Value $Value)
    (func $json-value.parse $parse)
    (func $json-value.stringify $stringify)
    (func $json-value.get $get)
    (func $json-value.as-number $as-number)
    (func $string.parse-s64 $integer))
  ;; Parsing and AST serialization both execute in WLT / Wasm GC.
  ;; The host only reads UTF-8, invokes this export, and prints the result.
  (func $normalize
    (export "normalize")
    (param $source $text)
    (result (%inst $result $text i32))
    (%effect reads writes allocates)
    (%result.do
      ($value (call $parse (local.get $source)))
      (%result.ok (call $stringify (local.get $value))))
  )
  ;; An example of inspecting the opaque AST through its public functions.
  (func $answer
    (param $value (ref $Value))
    (result i32)
    (%effect pure)
    (%match (call $get (local.get $value) (%text.lit "answer"))
      (case none () (i32.const 0))
      (case some ($field)
        (%match (call $as-number (local.get $field))
          (case none () (i32.const 0))
          (case some ($lexeme)
            (%match (call $integer (local.get $lexeme))
              (case none () (i32.const 0))
              (case some ($n) (i32.wrap_i64 (local.get $n))))
          ))))
  )

  (func $main
    (export "main")
    (result i32)
    (%effect reads writes allocates)
    (%match
      (call $parse (%text.lit "{\"answer\":42,\"items\":[null,true,\"日本語🦀\"]}"))
      (case err ($offset) (i32.const 0))
      (case ok ($value) (call $answer (local.get $value))))
  ))
