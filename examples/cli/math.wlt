(module
  (%use "wlt:core" (type $option))
  (%export (func $answer))

  (func $answer (result i32) (%effect pure)
    (%let $value (%inst $option i32) (%option.some 42)
      (%match $value
        (case some ($n) $n)
        (case none () 0)))))
