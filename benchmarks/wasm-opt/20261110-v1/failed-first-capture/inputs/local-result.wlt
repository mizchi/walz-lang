(module (%use "wlt:core" (type $result)) (func $main (export "main") (result i32) (%effect pure) (%let $x (%result.ok i32 i32 42) (%match $x (case ok ($x) $x) (case err ($e) $e)))))
