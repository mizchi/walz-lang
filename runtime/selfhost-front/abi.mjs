// Bootstrap ABI correctness v9: checked Core record/tuple views.
export const programs=[
  ['abi-record-scalars',`(module
    (type $Point (struct (field $x i32) (field $y i64)))
    (@abi.type $Point (record "x" "y"))
    (func $entry (export "f") (result i32)
      (@let $p (struct.new $Point 20 (i64.const 22))
        (+ (struct.get $Point $x $p) (i32.wrap_i64 (struct.get $Point $y $p))))))`,42],
  ['abi-tuple',`(module (type $Pair (struct (field i32) (field i64)))
    (@abi.type $Pair (tuple))
    (func $entry (export "f") (result i32)
      (@let $p (struct.new $Pair 20 (i64.const 22))
        (+ (struct.get $Pair 0 $p) (i32.wrap_i64 (struct.get $Pair 1 $p))))))`,42],
  ['abi-nested-record',`(module
    (type $Inner (struct (field $value i32))) (@abi.type $Inner (record "value"))
    (type $Outer (struct (field $inner (ref $Inner)))) (@abi.type $Outer (record "inner"))
    (func $entry (export "f") (result i32)
      (struct.get $Inner $value (struct.get $Outer $inner (struct.new $Outer (struct.new $Inner 42))))))`,42],
  ['abi-core-carriers',`(module
    (@use "walz:core" (type $bytes) (type $string) (type $list) (type $option) (type $result) (type $tuple))
    (type $Data (struct (field $blob $bytes) (field $label $string)
      (field $items $list<i32>) (field $maybe $option<i32>)
      (field $outcome $result<i32,i64>) (field $pair $tuple<i32,i64>)))
    (@abi.type $Data (record "blob" "label" "items" "maybe" "outcome" "pair"))
    (func $entry (export "f") (result i32)
      (@let $d (struct.new $Data (@bytes.new 42) "hi" (@list.new i32 42)
        (@option.some 42) (@result.ok 42) (@tuple.new 20 (i64.const 22)))
        (@bytes.get (struct.get $Data $blob $d) 0))))`,42],
  ['abi-value-alias',`(module (@use "walz:core" (type $list))
    (@type.alias $Items (@type.params $T) $list<$T>)
    (type $Data (struct (field $items $Items<i32>))) (@abi.type $Data (record "items"))
    (func $entry (export "f") (result i32)
      (@list.get i32 (struct.get $Data $items (struct.new $Data (@list.new i32 42))) 0)))`,42],
  ['abi-wat-field-prefix',`(module (type $R (struct (field $$value i32)))
    (@abi.type $R (record "value"))
    (func $entry (export "f") (result i32) (struct.get $R $$value (struct.new $R 42))))`,42],
  ['abi-core-import-alias',`(module
    (@use "walz:core" (type $bytes $Blob) (type $string $Text) (type $list $Items))
    (type $R (struct (field $blob $Blob) (field $label $Text) (field $items $Items<i32>)))
    (@abi.type $R (record "blob" "label" "items"))
    (func $entry (export "f") (result i32)
      (@let $r (struct.new $R (@bytes.new 42) "hi" (@list.new i32 42))
        (@list.get i32 (struct.get $R $items $r) 0))))`,42],

];
const record=(body,view='(record "value")')=>`(module (type $R ${body}) (@abi.type $R ${view}) (func $entry (export "f") (result i32) 42))`;
export const invalid=[
  ['abi-empty-record',record('(struct)','(record)'),225],
  ['abi-empty-tuple',record('(struct)','(tuple)'),225],
  ['abi-mutable-field',record('(struct (field $value (mut i32)))'),225],
  ['abi-mutable-tuple',record('(struct (field (mut i32)))','(tuple)'),225],
  ['abi-unnamed-field',record('(struct (field i32))'),225],
  ['abi-wrong-field-name',record('(struct (field $other i32))'),225],
  ['abi-invalid-name',record('(struct (field $Value i32))','(record "Value")'),225],
  ['abi-invalid-segment',record('(struct (field $bad--name i32))','(record "bad--name")'),225],
  ['abi-field-order',record('(struct (field $left i32) (field $right i32))','(record "right" "left")'),225],
  ['abi-non-string-name',record('(struct (field $value i32))','(record value)'),225],
  ['abi-duplicate-name',record('(struct (field $value i32) (field $other i32))','(record "value" "value")'),225],
  ['abi-duplicate-matching-labels',record('(struct (field $value i32) (field $value i32))','(record "value" "value")'),225],
  ['abi-wrong-target-kind',record('(array i32)'),225],
  ['abi-unknown-target','(module (@abi.type $Missing (record "value")) (func $entry (result i32) 42))',225],
  ['abi-malformed','(module (@abi.type) (func $entry (result i32) 42))',225],
  ['abi-duplicate-view','(module (type $R (struct (field $value i32))) (@abi.type $R (record "value")) (@abi.type $R (record "value")))',225],
  ['abi-recursive',record('(struct (field $value (ref $R)))'),225],
  ['abi-nullable',record('(struct (field $value (ref null $R)))'),225],
  ['abi-missing-child-view','(module (type $Child (struct (field $n i32))) (type $R (struct (field $value (ref $Child)))) (@abi.type $R (record "value")))',225],
  ['abi-list-cycle',`(module (@use "walz:core" (type $list))
    (type $R (struct (field $value $list<(ref $R)>))) (@abi.type $R (record "value")))`,225],
  ['abi-fake-bytes',`(module (type $bytes (struct (field $n i32)))
    (type $R (struct (field $value (ref $bytes)))) (@abi.type $R (record "value")))`,225],
  ['abi-alias-cycle',`(module (@type.alias $A $B) (@type.alias $B $A)
    (type $R (struct (field $value $A))) (@abi.type $R (record "value")))`,225],
  ['abi-raw-array',`(module (type $A (array i32))
    (type $R (struct (field $value (ref $A)))) (@abi.type $R (record "value")))`,225],
  ['abi-does-not-skip-body-check','(module (type $R (struct (field $value i32))) (@abi.type $R (record "value")) (func $entry (result i32) (i64.const 1)))',110],
];
export const projects=[['abi-imported-opaque-view',[
  ['main.walz','(module (@use "./lib.walz" (func $make) (func $read)) (func $entry (export "f") (result i32) (call $read (call $make))))'],
  ['lib.walz',`(module (@export (opaque $R) (func $make) (func $read))
    (type $R (struct (field $value i32))) (@abi.type $R (record "value"))
    (func $make (result (ref $R)) (struct.new $R 42))
    (func $read (param $r (ref $R)) (result i32) (struct.get $R $value $r)))`],
],42]];
export const badProjects=[['abi-foreign-opaque-view',[
  ['main.walz','(module (@use "./lib.walz" (type $R)) (@abi.type $R (record "value")))'],
  ['lib.walz','(module (@export (opaque $R)) (type $R (struct (field $value i32))))'],
],202]];
for(const [name,view] of [['wrong-label','(record "wrong")'],['tuple','(tuple)']])badProjects.push([
  `abi-foreign-opaque-${name}`,[
    ['main.walz',`(module (@use "./lib.walz" (type $R)) (@abi.type $R ${view}))`],
    ['lib.walz','(module (@export (opaque $R)) (type $R (struct (field $value i32))))'],
  ],202,
]);
badProjects.push(['abi-provider-range',[
  ['main.walz','(module (@use "./lib.walz" (func $read)) (func $entry (export "f") (result i32) (call $read)))'],
  ['lib.walz','(module ;; 日本語\n (@export (func $read)) (type $R (struct (field $value i32))) (@abi.type $R (record "wrong")) (func $read (result i32) (struct.get $R $value (struct.new $R 42))))'],
],225]);
projects.push(['abi-distinct-module-targets',[
  ['main.walz','(module (@use "./left.walz" (func $read $left)) (@use "./right.walz" (func $read $right)) (func $entry (export "f") (result i32) (+ (call $left) (call $right))))'],
  ['left.walz','(module (@export (func $read)) (type $R (struct (field $value i32))) (@abi.type $R (record "value")) (func $read (result i32) (struct.get $R $value (struct.new $R 20))))'],
  ['right.walz','(module (@export (func $read)) (type $R (struct (field $value i64))) (@abi.type $R (record "value")) (func $read (result i32) (i32.wrap_i64 (struct.get $R $value (struct.new $R (i64.const 22))))))'],
],42]);
export function diamond(level){
  return `(module (type $R0 (struct (field $value i32))) (@abi.type $R0 (record "value"))
    ${Array.from({length:level},(_,i)=>`(type $R${i+1} (struct (field (ref $R${i})) (field (ref $R${i})))) (@abi.type $R${i+1} (tuple))`).join('\n')}
    (func $entry (export "f") (result i32) 42))`;
}
programs.push(['abi-shared-diamond',diamond(10),42]);
invalid.push(['abi-flat-limit',diamond(11),226]);
export function chain(count){
  return `(module (type $R0 (struct (field $value i32))) (@abi.type $R0 (record "value"))
    ${Array.from({length:count-1},(_,i)=>`(type $R${i+1} (struct (field $inner (ref $R${i})))) (@abi.type $R${i+1} (record "inner"))`).join('\n')}
    (func $entry (export "f") (result i32) 42))`;
}
programs.push(['abi-depth-boundary',chain(63),42]);
invalid.push(['abi-depth-limit',chain(64),226]);
export function listTree(level,copies=0){
  return `(module (@use "walz:core" (type $list))
    (type $R0 (struct (field $value i32))) (@abi.type $R0 (record "value"))
    ${Array.from({length:level},(_,i)=>`(type $R${i+1} (struct (field $left $list<(ref $R${i})>) (field $right $list<(ref $R${i})>))) (@abi.type $R${i+1} (record "left" "right"))`).join('\n')}
    ${Array.from({length:copies},(_,i)=>`(type $Copy${i} (struct (field (ref $R${level})))) (@abi.type $Copy${i} (tuple))`).join('\n')}
    (func $entry (export "f") (result i32) 42))`;
}
invalid.push(['abi-expanded-shape-limit',listTree(12),226]);
invalid.push(['abi-total-shape-limit',listTree(8,600),226]);
invalid.push(['abi-field-limit',record(`(struct ${'(field i32) '.repeat(1025)})`,'(tuple)'),226]);
export const unsupported=[
  ['abi-newtype-view','(module (type $U (@newtype i32)) (@abi.type $U (u32)))',225],
  ['abi-generic-view','(module (type $Box (@type.params $T) (struct (field $value $T))) (@abi.type $Box (record "value")))',225],
  ['abi-heap-alias-view',`(module (type $R (struct (field $value i32))) (@abi.type $R (record "value"))
    (@type.alias $Alias $R) (type $Outer (struct (field $value (ref $Alias)))) (@abi.type $Outer (record "value")))`,225],
  ['abi-misplaced-view','(module (rec (type $R (struct (field $value i32))) (@abi.type $R (record "value"))))',225],
];
