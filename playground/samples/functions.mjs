export const functions = {
  closures: {
    category: 'Functions and modules', label: 'Closures with explicit captures',
    description: 'Declare a nominal closure type and every captured variable. Construction supplies those values explicitly; there are no implicit captures.',
    features: ['closures'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (@closure $Adder (@capture $base i64) (param $x i64) (result i64)
    (@effect pure)
    (+ $base $x))
  (func $main (export "main") (result i64) (@effect allocates)
    (@let $add40 (@closure.new $Adder 40)
      (@closure.call $Adder $add40 2))))`,
  },
  generics: {
    category: 'Functions and modules', label: 'Generic functions and type-class bounds',
    description: 'Ordinary calls infer unique type arguments from arguments and the expected result. @where checks primitive type-class constraints; inferred and explicit calls share monomorphized instances.',
    features: ['generics', 'generic-inference', 'type-bounds'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (func $add (@type.params $T) (@where (Add $T) (shared $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (@effect pure)
    ;; The expected i64 result fixes T; 40 and 2 are also i64.
    ;; Explicit form: (call $add<i64> 40 2), also (@inst $add i64).
    (call $add 40 2)))`,
  },
  'angle-generics': {
    category: 'Functions and modules', label: 'Angle type arguments',
    description: 'Write nested types as $result<$tuple<i32, i32>, i32> and specialize calls with $id<i32>. These spellings use the same type checking and monomorphization as @inst.',
    features: ['generics', 'angle-applications', 'result', 'tuples'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (@use "walz:core" (type $result) (type $tuple))
  (func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
  (func $pair (result $result<$tuple<i32, i32>, i32>) (@effect pure)
    (@result.ok (@tuple.new (i32.const 40) (i32.const 2))))
  (func $main (export "main") (result i32) (@effect pure)
    (@match (call $pair)
      (@case err ($code) $code)
      (@case ok ($value) (@let ($a $b) $value (call $id<i32> (+ $a $b)))))))`,
  },
  modules: {
    category: 'Functions and modules', label: 'File modules, selective uses and opaque types',
    description: 'One file is one module. Select exports using an explicit relative path. This single-file playground reports E_SOURCE_REQUIRED; run the two files below with the CLI.',
    features: ['modules', 'opaque'], expectation: { kind: 'diagnostic', code: 'E_SOURCE_REQUIRED' },
    source: `(module
  ;; Save this as main.walz. In user.walz, write:
  ;; (module
  ;;   (@export (opaque $Id) (func $new) (func $number))
  ;;   (type $Id (@newtype i32))
  ;;   (func $new (param $n i32) (result $Id) (@effect pure)
  ;;     (@newtype.wrap $Id $n))
  ;;   (func $number (param $id $Id) (result i32) (@effect pure)
  ;;     (@newtype.unwrap $Id $id)))
  (@use "./user.walz" (func $new $make-id) (func $number))
  (func $main (export "main") (result i32) (@effect pure)
    (call $number (call $make-id 42))))`,
  },
  'component-imports': {
    category: 'Functions and modules', label: 'Binary Component constructors (CLI)',
    description: 'Select a binary Component constructor with @use, instantiate it explicitly, and connect checked ports with @connect. The CLI snapshots the binary; this single-file playground cannot resolve relative files.',
    features: ['modules', 'component-imports', 'effects'],
    expectation: { kind: 'diagnostic', code: 'E_SOURCE_REQUIRED' },
    source: `(component
  ;; math.wit describes a closed library:
  ;; package example:math;
  ;; interface math { add: func(a: s32, b: s32) -> s32; }
  ;; world library { export math; }
  ;;
  ;; math.walz implements the exported function:
  ;; (module
  ;;   (func $add (export "math-add") (param $a i32) (param $b i32)
  ;;     (result i32) (@effect pure) (+ $a $b)))
  ;;
  ;; Build a Canonical ABI library with the CLI:
  ;; walz build math.walz --target component --wit-input math.wit --world library -o math.wasm
  ;; Then build this file with --target component.
  ;; WIT does not prove purity, so the call has a host effect.
  (@use "./math.wasm" (component $Math))
  (instance $math (instantiate $Math))
  (alias export $math "example:math/math" (instance $api))
  (@lib
    (import "math" "add" (func $add (param $a i32) (param $b i32) (result i32)))
    (func $main (result i32) (@effect host) (call $add 20 22)))
  (@connect "math" (instance $api))
  (@expose "main" $main))`,
  },
  ownership: {
    category: 'Functions and modules', label: 'Owned values and once-call closures',
    description: 'An owned box must be moved and consumed. Capturing an owned value makes this closure callable once; a shared GC reference alone does not imply ownership.',
    features: ['ownership', 'closures'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (@use "walz:core" (type $box))
  (@closure $Take (@capture $ticket (@inst $box i64)) (result i64)
    (@effect pure)
    (@match (@move $ticket)
      (@case own ($value) $value)))
  (func $main (export "main") (result i64) (@effect allocates)
    (@let $ticket (@box.new i64 42)
      (@let $take (@closure.new $Take (@move $ticket))
        (@closure.call $Take (@move $take))))))`,
  },
};
