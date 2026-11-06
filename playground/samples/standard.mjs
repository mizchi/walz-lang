export const standard = {
  lists: {
    category: 'Core library', label: 'Immutable lists, map and fold',
    description: 'Pass concrete closure types to generic map/fold. The core library checks callable signatures and compiles direct specialized calls.',
    features: ['lists', 'generics', 'closures', 'type-bounds'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core" (type $list) (func $list.map) (func $list.fold))
  (%closure $Scale (%capture $factor i64) (param $x i64) (result i64)
    (%effect pure) (* $x $factor))
  (%closure $Sum (param $total i64) (param $x i64) (result i64)
    (%effect pure) (+ $total $x))
  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (%let $scaled
      (call (%inst $list.map i64 i64 $Scale)
        (%list.new i64 1 2 3) (%closure.new $Scale 7))
      (call (%inst $list.fold i64 i64 $Sum) $scaled (%closure.new $Sum) 0))))`,
  },
  strings: {
    category: 'Core library', label: 'UTF-8 strings and bytes',
    description: 'Strings contain validated UTF-8; bytes are immutable binary data. Byte ranges are explicit, and decoding invalid UTF-8 produces none.',
    features: ['strings', 'bytes', 'option'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (%use "wlt:core" (type $option) (type $string))
  (func $main (export "main") (result i32) (%effect pure)
    (%let (($raw (%string.bytes (%string.lit "*hello")))
           ($head (%bytes.slice $raw 0 1)))
      (%match (%string.from-bytes $head)
        (case none () -1)
        (case some ($text) (%bytes.get (%string.bytes $text) 0))))))`,
  },
  views: {
    category: 'Core library', label: 'Zero-copy string and byte views',
    description: 'A view keeps the backing data and a byte range. Slicing shares storage; materialize with to-string only when a separate value is needed.',
    features: ['views', 'strings', 'bytes'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (%use "wlt:core"
    (func $string-view.new) (func $string-view.slice)
    (func $string-view.bytes) (func $bytes-view.get))
  (func $main (export "main") (result i32) (%effect allocates)
    (%let (($whole (call $string-view.new (%string.lit "hello:*")))
           ($tail (call $string-view.slice $whole 6 1)))
      (call $bytes-view.get (call $string-view.bytes $tail) 0))))`,
  },
  'array-views': {
    category: 'Core library', label: 'Generic zero-copy array views',
    description: 'Supply the element type and the nominal GC array type. A read-only view shares its storage: source mutations remain visible. Use to-list to copy the visible elements.',
    features: ['array-views', 'views', 'arrays', 'generics'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core"
    (func $array-view.new) (func $array-view.slice) (func $array-view.get))
  (type $Numbers (array (mut i64)))
  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (%let (($source (array.new_fixed $Numbers 4 99 40 0 99))
           ($whole (call (%inst $array-view.new i64 $Numbers) $source))
           ($part (call (%inst $array-view.slice i64 $Numbers) $whole 1 2)))
      (block (result i64)
        (array.set $Numbers $source 2 2)
        (+ (call (%inst $array-view.get i64 $Numbers) $part 0)
           (call (%inst $array-view.get i64 $Numbers) $part 1))))))`,
  },
  dict: {
    category: 'Core library', label: 'Dictionaries with typed values',
    description: 'Select generic core functions explicitly. Dictionaries are mutable opaque GC objects; get and set return Option values.',
    features: ['opaque', 'option'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core" (func $dict.new) (func $dict.set) (func $dict.get))
  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (%let $scores (call (%inst $dict.new i64))
      (block (result i64)
        (drop (call (%inst $dict.set i64) $scores (%string.lit "answer") 42))
        (%match (call (%inst $dict.get i64) $scores (%string.lit "answer"))
          (case none () -1)
          (case some ($score) $score))))))`,
  },
  json: {
    category: 'Core library', label: 'JSON parsing and typed access',
    description: 'The $json namespace provides parsing and accessors. A parse can fail with Result; indexing and conversion can be absent with Option.',
    features: ['json', 'result', 'option'], expectation: { kind: 'run', output: '1' },
    source: `(module
  (%use "wlt:core" (func $json.parse) (func $json.at) (func $json.as-boolean))
  (func $main (export "main") (result i32) (%effect reads writes allocates)
    (%match (call $json.parse (%string.lit "[true,false]"))
      (case err ($offset) -1)
      (case ok ($array)
        (%match (call $json.at $array 0)
          (case none () -2)
          (case some ($first)
            (%match (call $json.as-boolean $first)
              (case none () -3)
              (case some ($value) $value))))))))`,
  },
  simd: {
    category: 'Core library', label: 'SIMD with v128',
    description: 'Use standard Wasm SIMD operations and lane immediates. Keep v128 inside Wasm and return a scalar to JavaScript.',
    features: ['simd'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (func $main (export "main") (result i32) (%effect pure)
    (%let $values (type v128) (v128.const i32x4 40 10 20 30)
      (i32x4.extract_lane 0
        (i32x4.add $values (i32x4.splat 2))))))`,
  },
};
