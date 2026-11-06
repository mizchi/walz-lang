(module
  (%import "wlt:std"
    (type $string)
    (type $string-view)
    (type $result)
    (type $json-value)
    (func $string-view.new)
    (func $string-view.slice)
    (func $string-view.to-string)
    (func $json-value.parse))

  ;; The iteration index changes the input. There is no host call per iteration.
  (%bench "integer mix"
    (param $i i32)
    (result i32)
    (^ (* $i 16777619) (>>> $i 7)))

  ;; Return the view to keep the computed GC result observable.
  (%bench "string-view slice"
    (result (ref $string-view))
    (call $string-view.slice
      (call $string-view.new (%string.lit "prefix:日本語:suffix"))
      7 9))

  (%bench "string-view copy"
    (result $string)
    (call $string-view.to-string
      (call $string-view.slice
        (call $string-view.new (%string.lit "prefix:日本語:suffix"))
        7 9)))

  ;; Includes source creation and parsing, including error/result construction.
  (%bench "JSON parse"
    (result (%inst $result (ref $json-value) i32))
    (call $json-value.parse
      (%string.lit "{\"name\":\"Alice\",\"age\":30,\"active\":true,\"scores\":[1,2,3]}"))))
