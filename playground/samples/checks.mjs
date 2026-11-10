export const checks = {
  testing: {
    category: 'Checks and interop', label: 'Tests and benchmarks',
    description: '@test checks a predicate; @bench declares a measured expression. Compile their bodies here, or use walz test / walz bench in the CLI for their dedicated harnesses.',
    features: ['tests', 'benchmarks'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (func $answer (result i32) (@effect pure) (+ 40 2))
  (@test "answer is 42" (= (call $answer) 42))
  (@bench "compute the answer" (result i32) (call $answer))
  (func $main (export "main") (result i32) (@effect pure)
    (call $answer)))`,
  },
  'type-error': {
    category: 'Checks and interop', label: 'Diagnostic: a type mismatch',
    description: 'Intentional error: an i32 value cannot satisfy an i64 result. The diagnostic identifies the expression and expected type without an implicit conversion.',
    features: ['diagnostics'], expectation: { kind: 'diagnostic', code: 'E_TYPE_MISMATCH' },
    source: `(module
  (func $main (export "main") (result i64)
    (i32.const 42)))`,
  },
  'ownership-error': {
    category: 'Checks and interop', label: 'Diagnostic: using a moved value',
    description: 'Intentional error: an owned value cannot be moved twice. Inspect the related location pointing to the first move.',
    features: ['diagnostics', 'ownership'], expectation: { kind: 'diagnostic', code: 'E_OWNERSHIP_MOVED' },
    source: `(module
  (@use "walz:core" (type $box))
  (func $main (export "main")
    (@let $ticket (@box.new i64 42)
      (block
        (@box.close (@move $ticket))
        (@box.close (@move $ticket))))))`,
  },
  'effect-error': {
    category: 'Checks and interop', label: 'Diagnostic: an effect contract',
    description: 'Intentional error: pure is an upper bound on semantic effects. Mutable field writes are checked before optimization.',
    features: ['diagnostics', 'effects'], expectation: { kind: 'diagnostic', code: 'E_EFFECT_CONTRACT' },
    source: `(module
  (type $Cell (struct (field $value (mut i32))))
  (func $main (export "main") (@effect pure)
    (@let $cell (struct.new $Cell 0)
      (struct.set $Cell $value $cell 42))))`,
  },
  laws: {
    category: 'Checks and interop', label: 'Laws and the native proof kernel',
    description: 'A law quantifies over all inputs, including integer overflow. Browser compilation refuses to skip the external Lean proof step; use the native CLI to verify this program.',
    features: ['laws'], expectation: { kind: 'diagnostic', code: 'E_HOST_UNAVAILABLE' },
    source: `(module
  (@law "addition is commutative" (param $x i32) (param $y i32)
    (= (+ $x $y) (+ $y $x)))
  (func $main (export "main") (result i32) (@effect pure)
    (+ 40 2)))`,
  },
  native: {
    category: 'Checks and interop', label: 'Native WAT and multiple results',
    description: 'Standard WAT is accepted unchanged, including stack syntax and multiple results. Features outside the typed extension core receive opaque analysis.',
    features: ['native-wat'], expectation: { kind: 'run', output: '[7, 42n]' },
    source: `(module
  (func (export "main") (result i32 i64)
    i32.const 7
    i64.const 42))`,
  },
};
