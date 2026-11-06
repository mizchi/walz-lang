export const functions = {
  closures: {
    category: 'Functions and modules', label: 'Closures with explicit captures',
    description: 'Declare a nominal closure type and every captured variable. Construction supplies those values explicitly; there are no implicit captures.',
    features: ['closures'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%closure $Adder (%capture $base i64) (param $x i64) (result i64)
    (%effect pure)
    (+ $base $x))
  (func $main (export "main") (result i64) (%effect allocates)
    (%let $add40 (%closure.new $Adder 40)
      (%closure.call $Adder $add40 2))))`,
  },
  generics: {
    category: 'Functions and modules', label: 'Generic functions and type-class bounds',
    description: 'Instantiate type arguments explicitly. %where checks primitive type-class constraints, and each concrete function is monomorphized.',
    features: ['generics', 'type-bounds'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (func $add (%type.params $T) (%where (Add $T) (shared $T))
    (param $a $T) (param $b $T) (result $T) (%effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (%effect pure)
    (call (%inst $add i64) 40 2)))`,
  },
  modules: {
    category: 'Functions and modules', label: 'File modules, selective uses and opaque types',
    description: 'One file is one module. Select exports using an explicit relative path. This single-file playground reports E_SOURCE_REQUIRED; run the two files below with the CLI.',
    features: ['modules', 'opaque'], expectation: { kind: 'diagnostic', code: 'E_SOURCE_REQUIRED' },
    source: `(module
  ;; Save this as main.wlt. In user.wlt, write:
  ;; (module
  ;;   (%export (opaque $Id) (func $new) (func $number))
  ;;   (type $Id (%newtype i32))
  ;;   (func $new (param $n i32) (result $Id) (%effect pure)
  ;;     (%newtype.wrap $Id $n))
  ;;   (func $number (param $id $Id) (result i32) (%effect pure)
  ;;     (%newtype.unwrap $Id $id)))
  (%use "./user.wlt" (func $new $make-id) (func $number))
  (func $main (export "main") (result i32) (%effect pure)
    (call $number (call $make-id 42))))`,
  },
  ownership: {
    category: 'Functions and modules', label: 'Owned values and once-call closures',
    description: 'An owned box must be moved and consumed. Capturing an owned value makes this closure callable once; a shared GC reference alone does not imply ownership.',
    features: ['ownership', 'closures'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core" (type $box))
  (%closure $Take (%capture $ticket (%inst $box i64)) (result i64)
    (%effect pure)
    (%match (%move $ticket)
      (case own ($value) $value)))
  (func $main (export "main") (result i64) (%effect allocates)
    (%let $ticket (%box.new i64 42)
      (%let $take (%closure.new $Take (%move $ticket))
        (%closure.call $Take (%move $take))))))`,
  },
};
