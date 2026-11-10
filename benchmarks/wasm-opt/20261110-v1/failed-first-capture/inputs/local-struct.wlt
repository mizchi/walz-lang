(module (type $S (struct (field $n i32))) (func $main (export "main") (result i32) (%effect pure) (%let $x (struct.new $S 42) (struct.get $S $n $x))))
