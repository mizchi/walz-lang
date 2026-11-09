(module
  (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name (type i64) ?value) ...) ?body)))
  (%macro $delegated
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%$bindings ((?name ?value) ...) ?body)))
  (func $composed (export "composed") (param $a i64) (param $b i64) (result i64) (%effect pure)
    (%$delegated (($x $a) ($y $b)) (+ $x $y)))
  (%use "wlt:core" (type $option))
  (func $id (%type.params $T) (param $x $T) (result $T) (%effect pure) $x)
  (%macro $unwrap
    (rule (?value:expr)
      (infer (?T (option.element ?value)))
      (%match (%inst $option ?T) (result ?T) ?value
        (case none () (i64.const 0))
        (case some ($value) (call $id $value)))))
  (%macro $first
    (rule (?values:expr)
      (infer (?T (array.element ?values)) (?A (ref.heap ?values)))
      (%let $values ?values (call $id (array.get ?A $values 0)))))
  (type $Numbers (array i64))
  (func $unwrap (export "unwrap") (param $value (%inst $option i64)) (result i64) (%effect pure)
    (%$unwrap $value))
  (func $first (export "first") (param $value i64) (result i64) (%effect reads allocates)
    (%$first (array.new_fixed $Numbers 1 $value)))
  (%macro $identity
    (rule (?value:expr)
      (infer (?T ?value))
      (%let $saved (type ?T) ?value $saved)))
  (%macro $with
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body)
      (%let ?name ?value ?body)))
  (%macro $twice
    (rule (?value:expr)
      (when (Add ?value))
      (%let $saved ?value (+ $saved $saved))))
  (func $main (export "main") (result i32) (%effect pure)
    (%$with $saved (i32.const 21) (%$twice $saved)))
  (func $wide (export "wide") (param $value i64) (result i64) (%effect pure)
    (%$identity (%$twice $value)))
  (func $text (export "text") (param $value %string) (result %string) (%effect pure)
    (%$identity (call $id $value))))
