// Frontend corpus v6: layout reuse and sparse GC dependency ordering.
export function fields(count) {
  return `(module (@use "walz:core" (type $unit))
    (type $S (struct ${Array.from({length:24},(_,i)=>`(field $f${i} i32)`).join(' ')}))
    (func $entry (export "f") (result i32)
      (@let $s (struct.new $S ${Array.from({length:24},()=>42).join(' ')})
        (block (result i32)
          ${Array.from({length:count},()=>'(drop (@let $value (struct.get $S $f23 $s) $value))').join('\n')}
          (struct.get $S $f23 $s)))))`;
}
export function types(count) {
  return `(module (@use "walz:core" (type $unit))
    ${Array.from({length:count},(_,i)=>`(type $T${i} (struct (field $v i32)))`).join('\n')}
    (func $entry (export "f") (result i32) (struct.get $T${count-1} $v (struct.new $T${count-1} 42))))`;
}
export function recursive(count) {
  return `(module
    (rec ${Array.from({length:count},(_,i)=>`(type $T${i} (struct (field $next (ref null $T${(i+1)%count}))))`).join('\n')})
    (func $entry (export "f") (result i32)
      (i32.add (i32.const 41) (ref.is_null (struct.get $T0 $next (struct.new_default $T0))))))`;
}
export const programs = [
  ['layout-many-field-reads', fields(1000), 42],
  ['gc-order-more-than-128-types', types(300), 42],
  ['gc-order-large-explicit-rec', recursive(192), 42],
];
export const invalid = [];

export function aliases(count) {
  return `(module ${Array.from({length:count},(_,i)=>`(@type.alias $A${i} i32)`).join(' ')}
    (func $entry (export "f") (result i32) 42))`;
}
programs.push(['layout-generic-arguments', `(module
  (type $Box (@type.params $T) (struct (field $value $T)))
  (func $entry (export "f") (result i32)
    (@let (($a (struct.new $Box<i32> 20)) ($b (struct.new $Box<i64> (i64.const 22)))
           ($x (struct.get $Box<i32> $value $a)) ($y (struct.get $Box<i64> $value $b)))
      (+ $x (i32.wrap_i64 $y)))))`, 42]);
programs.push(['layout-nominal-declarations', `(module (@use "walz:core" (type $unit))
  (type $A (struct (field $value i32))) (type $B (struct (field $value i64)))
  (func $entry (export "f") (result i32)
    (@let (($a (struct.new $A 20)) ($b (struct.new $B (i64.const 22)))
           ($x (struct.get $A $value $a)) ($y (struct.get $B $value $b))) (+ $x (i32.wrap_i64 $y)))))`,42]);
programs.push(['gc-order-recursive-supertype', `(module
  (rec (type $Base (sub (struct (field $next (ref null $Child)))))
       (type $Child (sub final $Base (struct (field $next (ref null $Child))))))
  (func $entry (export "f") (result i32)
    (i32.add (i32.const 41) (ref.is_null (struct.get $Base $next (struct.new $Child (ref.null $Child)))))))`,42]);
invalid.push(['gc-order-parent-cycle', `(module (@use "walz:core" (type $unit))
  (type $A (sub $B (struct))) (type $B (sub $A (struct)))
  (func $entry (result i32) 42))`,118]);
invalid.push(['gc-order-preserves-explicit-group-boundary', `(module (@use "walz:core" (type $unit))
  (rec (type $A (struct (field (ref null $B)))) (type $B (struct (field (ref null $C)))))
  (type $C (struct (field (ref null $A)))) (func $entry (result i32) 42))`,118]);
invalid.push(['layout-rejects-different-field-types', `(module
  (type $Box (@type.params $T) (struct (field $value $T)))
  (func $entry (result i32)
    (@let (($a (struct.new $Box<i32> 20)) ($b (struct.new $Box<i64> (i64.const 22)))
           ($x (struct.get $Box<i32> $value $a)) ($y (struct.get $Box<i64> $value $b))) (+ $x $y))))`,110]);
export const boundedInvalid = [['concrete-instance-cache-limit', aliases(3073), 150], ['gc-type-pool-limit',types(1025),150]];

export const nativePrograms = [['gc-order-keeps-original-numeric-type-indices', `(module (@use "walz:core" (type $unit))
  (type $Z (struct (field (ref null $B)))) (type $A (struct (field i64))) (type $B (struct (field i32)))
  (func $entry (export "f") (result i32) (struct.get 2 0 (struct.new 2 42))))`,42]];
