export const data = {
  structs: {
    category: 'Data and types', label: 'Named struct construction',
    description: 'Name every field when constructing GC structs. Field-map syntax and literal elision are independent conveniences; field types remain checked.',
    features: ['structs', 'strings'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (%use "wlt:core" (type $string))
  (type $User (struct (field $name $string) (field $age i32)))
  (func $main (export "main") (result i32) (%effect allocates)
    (%let (($alice (%$User::{:name "Alice" :age 30}))
           ($bob (%struct.new $User ($age 12) ($name "Bob"))))
      (+ (struct.get $User $age $alice) (struct.get $User $age $bob)))))`,
  },
  arrays: {
    category: 'Data and types', label: 'GC arrays and bounds',
    description: 'Use Wasm GC arrays for mutable indexed storage. Reads, writes and lengths are explicit; out-of-bounds access is a Wasm trap.',
    features: ['arrays', 'loops'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (type $Numbers (array (mut i64)))
  (func $main (export "main") (result i64) (%effect reads writes allocates)
    (%let $xs (array.new_fixed $Numbers 3 10 20 0)
      (block (result i64)
        (array.set $Numbers $xs 2 12)
        (%loop $sum (bind $i i32 0) (bind $total i64 0) (result i64)
          (if (result i64) (< $i (array.len $xs))
            (then (%recur $sum (+ $i 1) (+ $total (array.get $Numbers $xs $i))))
            (else $total)))))))`,
  },
  enums: {
    category: 'Data and types', label: 'Enums and exhaustive matches',
    description: 'Declare a closed set of variants and match every case exactly once. Each case gives names to its typed payload.',
    features: ['enums'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (type $Reading (%variant (missing) (value i64)))
  (func $read (param $item (ref $Reading)) (result i64) (%effect pure)
    (%match $item
      (case missing () 0)
      (case value ($n) $n)))
  (func $main (export "main") (result i64) (%effect allocates)
    (call $read (%variant.new $Reading value 42))))`,
  },
  result: {
    category: 'Data and types', label: 'Core Result and GC',
    description: 'Import core value types explicitly. The result declaration gives the function type; $result is the success/error value type.',
    features: ['result'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core" (type $result))
  (func $main (export "main") (result i64) (%effect pure)
    (%let $answer (%inst $result i64 i32) (%result.ok 42)
      (%match $answer
        (case ok ($value) $value)
        (case err ($code) (i64.const 0))))))`,
  },
  'result-do': {
    category: 'Data and types', label: 'Result pipelines and error propagation',
    description: '%result.do evaluates stages once, from left to right, and stops at the first error. The error is the value of this expression.',
    features: ['result'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (%use "wlt:core" (type $result))
  (func $positive (param $n i32) (result (%inst $result i32 i32)) (%effect pure)
    (if (result (%inst $result i32 i32)) (< $n 0)
      (then (%result.err 1)) (else (%result.ok $n))))
  (func $main (export "main") (result i32) (%effect pure)
    (%let $answer (%result.do
        ($a (call $positive 40))
        ($b (call $positive 2))
        (%result.ok (+ $a $b)))
      (%match $answer
        (case ok ($n) $n)
        (case err ($code) (- 0 $code))))))`,
  },
  'option-tuples': {
    category: 'Data and types', label: 'Option, tuples and pattern bindings',
    description: 'Combine standard value types, name a concrete type with an alias, and destructure a tuple in a single %let.',
    features: ['option', 'tuples'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core" (type $option) (type $tuple))
  (%type.alias $Pair (%inst $tuple i64 i64))
  (func $main (export "main") (result i64) (%effect pure)
    (%let $pair $Pair (%tuple.new 40 2)
      (%let ($base $extra) $pair
        (%match (%option.some i64 $base)
          (case none () $extra)
          (case some ($n) (+ $n $extra)))))))`,
  },
  newtypes: {
    category: 'Data and types', label: 'Scalar newtypes and aliases',
    description: 'An alias preserves type identity. A newtype creates a distinct nominal type; wrap and unwrap are explicit and erase before Wasm emission.',
    features: ['scalars'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (type $UserId (%newtype i32))
  (%type.alias $Id $UserId)
  (func $next (param $id $Id) (result $UserId) (%effect pure)
    (%newtype.wrap $UserId (+ (%newtype.unwrap $UserId $id) 1)))
  (func $main (export "main") (result i32) (%effect pure)
    (%newtype.unwrap $UserId (call $next (%newtype.wrap $UserId 41)))))`,
  },
};
