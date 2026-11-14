// Frontend correctness corpus v5: checked snapshots of integer GC arrays.
const module = (storage, body, effect = 'reads writes allocates') => `(module
  (@use "walz:core" (type $bytes)) (type $A (array ${storage}))
  (func $entry (export "f") (result i32) (@effect ${effect}) ${body}))`;
export const programs = [
  ['bytes-from-i32-snapshot', module('(mut i32)', `(@let (($a (array.new_fixed $A 2 42 255)) ($b (@bytes.from-array $A $a)))
    (block (result i32) (array.set $A $a 0 7) (@bytes.get $b 0)))`), 42],
  ['bytes-from-immutable-i32', module('i32', '(@bytes.get (@bytes.from-array $A (array.new_fixed $A 2 42 255)) 0)', 'allocates'), 42],
  ['bytes-from-i16', module('(mut i16)', '(@bytes.get (@bytes.from-array $A (array.new_fixed $A 2 42 255)) 0)'), 42],
  ['bytes-from-i8-unsigned', module('i8', '(@bytes.get (@bytes.from-array $A (array.new_fixed $A 1 -1)) 0)', 'allocates'), 255],
  ['bytes-from-i32-selected-range', module('(mut i32)', '(@bytes.get (@bytes.from-array $A (array.new_fixed $A 3 -1 42 256) 1 1) 0)'), 42],
  ['bytes-from-i16-selected-range', module('(mut i16)', '(@bytes.get (@bytes.from-array $A (array.new_fixed $A 3 65535 42 256) 1 1) 0)'), 42],
  ['bytes-from-i32-empty-at-end', module('i32', '(@bytes.len (@bytes.from-array $A (array.new_fixed $A 1 -1) 1 0))', 'allocates'), 0],
];
for (const [name, storage, value] of [
  ['i32-negative', '(mut i32)', -1], ['i32-high', '(mut i32)', 256],
  ['i16-high', '(mut i16)', 256], ['i16-unsigned', '(mut i16)', -1],
]) programs.push([`bytes-from-${name}-traps`, module(storage, `(@bytes.len (@bytes.from-array $A (array.new_fixed $A 1 ${value})))`), 'trap']);
for (const [name, start, length] of [['offset', 2, 0], ['negative-offset', -1, 0], ['length', 0, 2], ['negative-length', 0, -1]])
  programs.push([`bytes-from-range-${name}-traps`, module('i32', `(@bytes.len (@bytes.from-array $A (array.new_fixed $A 1 42) ${start} ${length}))`, 'allocates'), 'trap']);
programs.push(['bytes-from-array-evaluation-order', `(module
  (@use "walz:core" (type $bytes))
  (type $A (array (mut i32))) (type $Counter (struct (field (mut i32))))
  (func $mark (param $c (ref $Counter)) (param $digit i32) (param $value i32) (result i32) (@effect reads writes)
    (block (result i32) (struct.set $Counter 0 $c (+ (* (struct.get $Counter 0 $c) 10) $digit)) $value))
  (func $entry (export "f") (result i32) (@effect reads writes allocates)
    (@let (($c (struct.new $Counter 0))
           ($b (@bytes.from-array $A
             (block (result (ref $A)) (drop (call $mark $c 1 0)) (array.new_fixed $A 3 -1 42 256))
             (call $mark $c 2 1) (call $mark $c 3 1))))
      (block (result i32) (drop (@bytes.get $b 0)) (struct.get $Counter 0 $c)))))`, 123]);
export const invalid = [
  ['bytes-from-i64-rejected', module('(mut i64)', '(@bytes.len (@bytes.from-array $A (array.new_fixed $A 1 (i64.const 42))))'), 110],
  ['bytes-from-array-mutable-read-bound', `(module (@use "walz:core" (type $bytes))
    (type $A (array (mut i32)))
    (func $read (param $a (ref $A)) (result i32) (@effect pure) (@bytes.len (@bytes.from-array $A $a)))
    (func $entry (result i32) (@effect reads allocates) (call $read (array.new_fixed $A 1 42))))`, 123],
  ['bytes-from-array-wrong-arity', module('i32', '(@bytes.len (@bytes.from-array $A (array.new_fixed $A 1 42) 0))', 'pure'), 106],
  ['bytes-from-array-null-rejected', module('i32', '(@bytes.len (@bytes.from-array $A (ref.null $A)))', 'pure'), 110],
];
export const badProjects = [['bytes-from-opaque-array', [
  ['main.walz', '(module (@use "walz:core" (type $bytes)) (@use "./secret.walz" (type $A)) (func $entry (param $a (ref $A)) (result $bytes) (@effect reads) (@bytes.from-array $A $a)))'],
  ['secret.walz', '(@lib (@export (opaque $A)) (type $A (array (mut i32))))'],
], 202]];

invalid.push(
  ['nonnull-let-rejects-null', '(module (type $A (array i32)) (func $entry (result i32) (@let $a (type (ref $A)) (ref.null $A) (i32.const 42))))', 110],
  ['nonnull-loop-initializer-rejects-null', '(module (@use "walz:core" (type $unit)) (type $A (array i32)) (func $entry (result i32) (@loop $copy (bind $a (ref $A) (ref.null $A)) (result i32) 42)))', 110],
  ['nonnull-recur-rejects-null', '(module (@use "walz:core" (type $unit)) (type $A (array i32)) (func $entry (result i32) (@loop $copy (bind $a (ref $A) (array.new_fixed $A 1 42)) (result i32) (@recur $copy (ref.null $A)))))', 110],
);
programs.push(
  ['nonnull-let-accepts-subtype', `(module
    (type $Base (sub (struct (field i32)))) (type $Derived (sub final $Base (struct (field i32))))
    (func $entry (export "f") (result i32) (@let $a (type (ref $Base)) (struct.new $Derived (i32.const 42)) (struct.get $Base 0 (local.get $a)))))`, 42],
  ['nonnull-loop-preserves-numeric-branch', `(module (@use "walz:core" (type $unit))
    (type $A (array i32)) (func $entry (export "f") (result i32)
      (block (result i32) (@loop $copy (bind $a (ref $A) (br 0 42)) (result i32) 7))))`, 42],
);

programs.push(['bytes-from-immutable-copy-is-pure', `(module (@use "walz:core" (type $bytes))
  (type $A (array i32))
  (func $read (param $a (ref $A)) (result i32) (@effect pure) (@bytes.get (@bytes.from-array $A $a) 0))
  (func $entry (export "f") (result i32) (@effect allocates) (call $read (array.new_fixed $A 1 42))))`, 42]);

// Rust's typed path requires named branches; retain the bootstrap's native
// numeric-depth regression separately from identical-input acceptance parity.
export const nativeBranchPrograms = programs.splice(programs.findIndex(([name]) => name === 'nonnull-loop-preserves-numeric-branch'), 1);
programs.push(['nonnull-loop-preserves-named-branch', `(module (@use "walz:core" (type $unit))
  (type $A (array i32)) (func $entry (export "f") (result i32)
    (block $exit (result i32) (@loop $copy (bind $a (ref $A) (br $exit 42)) (result i32) 7))))`, 42]);

for (const entry of invalid.filter(([name]) => name.includes('null'))) entry.push('(ref.null $A)');
invalid.find(([name]) => name === 'bytes-from-array-null-rejected')[1] =
  ';; UTF-8: 日本語 ✨\n' + invalid.find(([name]) => name === 'bytes-from-array-null-rejected')[1];
