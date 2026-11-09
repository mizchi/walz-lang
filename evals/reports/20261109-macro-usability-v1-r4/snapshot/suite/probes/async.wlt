(module
  (%use "wlt:core" (type $future))
  (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $ms i32)))
  (import "wlt:host/event-loop@0.1.0" "defer-i64"
    (func $defer (param $ms i32) (param $value i64) (result (%inst $future i64))))
  (%macro $await-let
    (rule (?name:ident ?pending:expr ?body:expr)
      (bind ?name ?body)
      (%let ?name (%future.await i64 ?pending) ?body)))
  (func $main (export "main") (param $ms i32) (result i64)
    (%effect host suspends)
    (%let $unread (call $defer $ms (i64.const 99))
      (%$await-let $value (call $defer $ms (i64.const 40))
        (block (result i64)
          (%future.close i64 (%move $unread))
          (+ $value 2)))))
  (func $queued (export "queued") (result i64) (%effect pure) (i64.const 7)))
