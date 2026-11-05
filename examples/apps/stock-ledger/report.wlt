(module
  (namespace $report
    (import "../../../stdlib/json.wlt"
      (type $json.Object)
      (func $json.boolean)
      (func $json.finish)
      (func $json.new)
      (func $json.string)
      (func $json.u64))
    (import "wlt:host/text-io@0.1.0" "write-line"
      (func $write (param $line text) (result (future (result unit i32))))
    )
    (public (func $success) (func $failure) (func $invalid))
    (func $error-name (param $e (ref $stock.Error)) (result text) (effects pure)
      (match $stock.Error (result text)
        (local.get $e)
        (case insufficient () (text.lit "insufficient-stock"))
        (case overflow () (text.lit "quantity-overflow"))
        (case capacity () (text.lit "capacity"))))

    (func $base (param $op text) (param $key text) (result (ref $json.Object))
      (effects reads writes allocates)
      (let $o (call $json.new)
        (block
          (result (ref $json.Object))
          (call $json.string (local.get $o) (text.lit "op") (local.get $op))
          (call $json.string (local.get $o) (text.lit "key") (local.get $key))
          (local.get $o))))

    (func $success (param $op text) (param $key text) (param $quantity i64)
      (result (result unit i32)) (effects reads writes allocates host suspends)
      (let $o (call $base (local.get $op) (local.get $key))
        (block
          (result (result unit i32))
          (call $json.boolean (local.get $o) (text.lit "ok") (i32.const 1))
          (call $json.u64 (local.get $o) (text.lit "quantity") (local.get $quantity))
          (future.await
            (result unit i32)
            (call $write (call $json.finish (local.get $o))))
        )))

    (func $failure (param $op text) (param $key text) (param $error (ref $stock.Error))
      (result (result unit i32)) (effects reads writes allocates host suspends)
      (let $o (call $base (local.get $op) (local.get $key))
        (block
          (result (result unit i32))
          (call $json.boolean (local.get $o) (text.lit "ok") (i32.const 0))
          (call $json.string
            (local.get $o)
            (text.lit "error")
            (call $error-name (local.get $error)))
          (future.await
            (result unit i32)
            (call $write (call $json.finish (local.get $o))))
        )))

    (func $invalid (result (result unit i32)) (effects host suspends)
      (future.await
        (result unit i32)
        (call $write (text.lit "{\"ok\":false,\"error\":\"invalid-command\"}"))))
  ))
