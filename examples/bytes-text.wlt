(module
  (func $main (export "main") (result i32) (%effects pure)
    (%let $raw (%text.bytes (%text.lit "*日本語"))
      (%let $head (%bytes.slice (local.get $raw) (i32.const 0) (i32.const 1))
        (%match (%option %text) (result i32) (%text.from-bytes (local.get $head))
          (case none () (i32.const -1))
          (case some ($text) (%bytes.get (%text.bytes (local.get $text)) (i32.const 0))))))))
