(module
  (%use "wlt:core" (type $option) (type $result) (func $option.ok-or))
  
  (func $read (param $value (%inst $option i64))
    (result (%inst $result i64 i32)) (%effect pure)
    (call $option.ok-or $value (i32.const 7)))
  (func $main (export "main") (result i64) (%effect pure)
    (%match (call $read (%option.some i64 42))
      (case ok ($n) $n) (case err ($e) (i64.extend_i32_s $e))))
  (func $fallback (export "fallback") (result i64) (%effect pure)
    (%match (call $read (%option.none i64))
      (case ok ($n) $n) (case err ($e) (- (i64.const 0) (i64.extend_i32_s $e))))))
