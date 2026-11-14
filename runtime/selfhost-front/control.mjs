// Maintained frontend correctness v10: control-result context and unit matches.
const option='(@use "walz:core" (type $option))';
const choose=value=>`(@let $input (type $option<i32>) (@option.some (i32.const ${value}))
  (@match $input (@case none () (nop)) (@case some ($v) (drop $v))))`;
export const programs=[
  ['control-empty-function-result',`(module ${option}
    (func $step (result) (nop))
    (func $entry (export "f") (result i32) (block (result i32) (call $step) 42)))`,42],
  ['control-inferred-string-bytes',`(module (@use "walz:core" (type $string))
    (func $length (param $from $string) (result i32)
      (@let $from (@string.bytes $from) (+ 40 (@bytes.len $from))))
    (func $entry (export "f") (result i32) (call $length "hi")))`,42],
  ['control-inferred-string-constructor',`(module (@use "walz:core" (type $string))
    (func $entry (export "f") (result i32)
      (@let $text (@string.lit "hi") (+ 40 (@bytes.len (@string.bytes $text))))))`,42],
  ['control-literal-is-not-local-index',`(module ${option}
    (type $Node (struct))
    (func $step (param $state (ref $Node)) (param $node (ref $Node)) (result i32)
      (@let $value (+ 1 (i32.const 41)) $value))
    (func $entry (export "f") (result i32) (call $step (struct.new $Node) (struct.new $Node))))`,42],
  ['control-variable-condition',`(module ${option}
    (func $step (param $ready i32) (result i32)
      (block (result i32) (if $ready (then (return 42)) (else (nop))) 0))
    (func $entry (export "f") (result i32) (+ (call $step 1) (call $step 0))))`,42],
  ['control-empty-result',`(module ${option}
    (func $step (block (result) (nop)))
    (func $entry (export "f") (result i32) (block (result i32) (call $step) 42)))`,42],
  ['control-unit-match',`(module ${option}
    (func $step (param $x $option<i32>)
      (@match $x (@case none () (nop)) (@case some ($v) (drop $v))))
    (func $entry (export "f") (result i32)
      (block (result i32) (call $step (@option.none)) (call $step (@option.some 1)) 42)))`,42],
  ['control-sequence-unit-match',`(module ${option}
    (func $entry (export "f") (result i32) (block (result i32) ${choose(1)} 42)))`,42],
  ['control-i64-condition',`(module ${option}
    (func $entry (export "f") (result i64) (if (result i64) 1 (then 42) (else 0))))`,42n],
  ['control-loop-unit-match',`(module ${option}
    (func $entry (export "f") (result i32)
      (loop $done (result i32) (block (result i32) ${choose(1)} (i32.const 42)))))`,42],
  ['control-unit-match-return',`(module ${option}
    (func $step (param $x $option<i32>)
      (@match $x (@case none () (return)) (@case some ($v) (drop $v))))
    (func $entry (export "f") (result i32)
      (block (result i32) (call $step (@option.none)) (call $step (@option.some 1)) 42)))`,42],
];
// The Rust typed @use path still requires named locals and single-expression
// branches and does not implement labelled if. Validate these against Wasm.
export const nativePrograms=[
  ['control-explicit-local-index',`(module ${option}
    (func $step (param $state i32) (param $value i64) (result i64)
      (@let $value (local.get 1) (+ $value 1)))
    (func $entry (export "f") (result i64) (call $step 0 41)))`,42n],
  ['control-named-if',`(module ${option}
    (func $step (param $ready i32) (result i32)
      (if $ready (result i32) (local.get $ready) (then (br $ready (i32.const 42))) (else (i32.const 0))))
    (func $entry (export "f") (result i32) (+ (call $step 1) (call $step 0))))`,42],
  ['control-branch-unit-match',`(module ${option}
    (func $entry (export "f") (result i32)
      (if (result i32) (i32.const 1) (then ${choose(1)} 42) (else 0))))`,42],
];
export const invalid=[
  ['control-string-literal-needs-type',`(module (@use "walz:core" (type $string))
    (func $entry (result i32) (@let $text "hi" (+ 40 (@bytes.len (@string.bytes $text))))))`,118],
  ['control-function-multiple-results',`(module ${option}
    (func $entry (result i32 i64) (i32.const 42)))`,104],
  ['control-skipped-unit-match-type',`(module ${option}
    (func $step (param $x $option<i32>)
      (@match $x (@case none () (drop (i64.add (i32.const 1) (i64.const 2)))) (@case some ($v) (drop $v))))
    (func $entry (result i32) (block (result i32) (call $step (@option.some 1)) 42)))`,110],
  ['control-i64-condition-mismatch',`(module ${option}
    (func $entry (result i64) (if (result i64) (i64.const 1) (then 42) (else 0))))`,110],
];
// These contracts are outside the single-result bootstrap subset; never truncate.
export const unsupported=[
  ['control-multiple-results',`(module ${option} (func $entry (result i32)
    (block (result i32 i64) (i32.const 42))))`,104],
];
for(const type of ['i32','i64'])for(const [op,expected] of [['&',0],['|',43],['^',43],['<<',84],['>>',21],['>>>',21]]){
  programs.push([`control-inferred-${type}-${op}`,
    `(module ${option} (func $entry (export "f") (result ${type})
      (@let $v (${op} (${type}.const 42) 1) $v)))`,type==='i64'?BigInt(expected):expected]);
}
for(const [op,expected] of [['=',0],['!=',1],['<',1],['>',0],['<=',1],['>=',0]]){
  programs.push([`control-inferred-i64-comparison-${op}`,
    `(module ${option} (func $entry (export "f") (result i32)
      (@let $v (${op} (i64.const 1) 2) $v)))`,expected]);
}
for(const type of ['i32','i64'])programs.push([`control-unsigned-shift-${type}`,
  `(module ${option} (func $entry (export "f") (result ${type})
    (@let $v (>>> (${type}.const -1) 1) $v)))`,type==='i64'?9223372036854775807n:2147483647]);
