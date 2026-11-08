(module
 (%use "wlt:core" (type $future) (type $stream) (type $string))
 (func $consume (export "api-consume") (param $f (%inst $future i32)) (result i32) (%effect suspends) (%future.await i32 (%move $f)))
 (func $ready (export "api-ready") (result (%inst $future i32)) (%effect allocates) (%future.ready i32 42))
 (func $prepend (export "api-prepend") (param $tail (%inst $stream i32)) (result (%inst $stream i32)) (%effect allocates) (%stream.cons i32 7 (%move $tail)))
 (func $text (export "api-ready-text") (param $v $string) (result (%inst $future $string)) (%effect allocates) (%future.ready $string $v))
)
