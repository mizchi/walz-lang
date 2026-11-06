export const basics = {
  arithmetic: {
    category: 'Basics', label: 'Bindings and arithmetic',
    description: 'Initialize immutable bindings from left to right. An explicit type gives bare literals their width; $x reads a local.',
    features: ['bindings', 'operators'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (func $main (export "main") (result i32) (%effect pure)
    (%let (($x (type i32) 40)
           ($step (+ $x 2)))
      $step)))`,
  },
  operators: {
    category: 'Basics', label: 'Logic, comparisons and bit shifts',
    description: 'Operators resolve to primitive Wasm instructions. Logical operators short-circuit and normalize their result to 0 or 1.',
    features: ['operators'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (func $main (export "main") (result i32) (%effect pure)
    (%let $value (type i32) (>>> (& 84 127) 1)
      ;; The division is never evaluated, but still type-checked.
      (if (result i32) (&& (= $value 42) (|| 1 (i32.div_s 1 0)))
        (then $value)
        (else 0)))))`,
  },
  scalars: {
    category: 'Basics', label: 'Floating point and explicit conversions',
    description: 'Use standard Wasm numeric instructions for f32/f64. Conversions are explicit; there is no implicit integer or float widening.',
    features: ['scalars'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (func $main (export "main") (result i32) (%effect pure)
    (%let (($root (type f64) (f64.sqrt 1764))
           ($narrow (type f32) (f32.demote_f64 $root)))
      (i32.trunc_f64_s (f64.promote_f32 $narrow)))))`,
  },
  control: {
    category: 'Control flow', label: 'If expressions and early exits',
    description: 'if and block produce typed values. return exits a function; br exits a named block with a value.',
    features: ['control-flow'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (func $checked (param $n i32) (result i32) (%effect pure)
    (block (result i32)
      (if (< $n 0) (then (return 0)) (else (nop)))
      (block $done (result i32)
        (if (= $n 42) (then (br $done $n)) (else (nop)))
        (+ $n 1))))
  (func $main (export "main") (result i32) (%effect pure)
    (call $checked 42)))`,
  },
  loop: {
    category: 'Control flow', label: 'Loops with explicit state',
    description: '%loop carries immutable state. %recur updates all bindings simultaneously, in tail position, without mutable locals.',
    features: ['loops'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (func $main (export "main") (result i64) (%effect pure)
    (%loop $sum (bind $i i64 0) (bind $total i64 0) (result i64)
      (if (result i64) (< $i 7)
        (then (%recur $sum (+ $i 1) (+ $total 6)))
        (else $total)))))`,
  },
  for: {
    category: 'Control flow', label: 'For loops and mutable GC fields',
    description: '%for provides a condition and step. A mutable GC cell holds the total; effects describe its reads and writes.',
    features: ['loops', 'structs', 'effects'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (type $Cell (struct (field $sum (mut i32))))
  (func $main (export "main") (result i32) (%effect reads writes allocates)
    (%let $cell (struct.new $Cell 0)
      (block (result i32)
        (%for $count (bind $i i32 0)
          (while (< $i 7)) (step (+ $i 1))
          (struct.set $Cell $sum $cell (+ (struct.get $Cell $sum $cell) 6)))
        (struct.get $Cell $sum $cell)))))`,
  },
  recursion: {
    category: 'Control flow', label: 'Mutually recursive functions',
    description: 'Functions may call declarations that appear later. Recursive calls use ordinary typed function signatures.',
    features: ['recursion'], expectation: { kind: 'run', output: '1' },
    source: `(module
  (func $even (param $n i32) (result i32) (%effect pure)
    (if (result i32) (= $n 0)
      (then 1) (else (call $odd (- $n 1)))))
  (func $odd (param $n i32) (result i32) (%effect pure)
    (if (result i32) (= $n 0)
      (then 0) (else (call $even (- $n 1)))))
  (func $main (export "main") (result i32) (%effect pure)
    (call $even 42)))`,
  },
  exceptions: {
    category: 'Control flow', label: 'Wasm exceptions and typed catches',
    description: '%try is a macro over Wasm exceptions. A catch binds the tag payload. Wasm traps such as division by zero are not caught.',
    features: ['control-flow', 'effects'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (tag $Missing (param i32))
  (func $lookup (result i32) (%effect (throws $Missing))
    (throw $Missing 42))
  (func $main (export "main") (result i32) (%effect pure)
    (%try (result i32)
      (do (call $lookup))
      (catch $Missing ($fallback) $fallback))))`,
  },
};
