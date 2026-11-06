(module
  (%import "wlt:std"
    (type $text)
    (type $json-value.Value $Value)
    (func $json-value.parse)
    (func $json-value.stringify))
  (type $Input (array (mut i32)))
  ;; These setup operations are excluded from the parse measurements.
  (func $new
    (export "input-new")
    (param $n i32)
    (result (ref $Input))
    (%effects allocates)
    (array.new $Input (i32.const 0) (local.get $n)))

  (func $set
    (export "input-set")
    (param $data (ref $Input))
    (param $i i32)
    (param $v i32)
    (%effects writes)
    (array.set $Input (local.get $data) (local.get $i) (local.get $v)))

  (func $text
    (export "input-text")
    (param $data (ref $Input))
    (result $text)
    (%effects reads)
    (%match (%text.from-bytes (%bytes.from-array $Input (local.get $data)))
      (case some ($text) (local.get $text))
      (case none () (unreachable))))
  ;; Return the actual GC AST to the host, preventing unused-result elimination.
  (func $parse
    (export "parse")
    (param $source $text)
    (result (ref $Value))
    (%effects allocates reads writes)
    (%match (call $json-value.parse (local.get $source))
      (case ok ($value) (local.get $value))
      (case err ($offset) (unreachable))))
  ;; Full serialization/hash is a preflight, never part of parse-only timing.
  (func $checksum
    (export "checksum")
    (param $value (ref $Value))
    (result i32)
    (%effects allocates reads writes)
    (%let $bytes (%text.bytes (call $json-value.stringify (local.get $value)))
      (%loop $hash
        (bind $i i32 (i32.const 0))
        (bind $hash i32 (i32.const -2128831035))
        (result i32)
        (if
          (result i32)
          (i32.ge_u (local.get $i) (%bytes.len (local.get $bytes)))
          (then (local.get $hash))
          (else
            (%recur $hash
              (i32.add (local.get $i) (i32.const 1))
              (i32.mul
                (i32.xor
                  (local.get $hash)
                  (%bytes.get (local.get $bytes) (local.get $i)))
                (i32.const 16777619))))
        ))))
)
