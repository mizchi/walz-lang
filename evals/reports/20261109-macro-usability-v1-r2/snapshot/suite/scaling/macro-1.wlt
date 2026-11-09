(module
 (%macro $double (rule (?x:expr) (infer (?T ?x)) (when (Add ?x)) (%let $value (type ?T) ?x (+ $value $value))))
 (func $f0 (export "f0") (param $x i64) (result i64) (%effect pure) (%$double $x))
)