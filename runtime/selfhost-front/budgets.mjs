// Maintained frontend corpus v4: cache entries count definitions, not calls.
export function functions(count) {
  return `(module (@use "walz:core" (type $unit))
    ${Array.from({length:count},(_,i)=>`(func $f${i} (result i32) (i32.const 42))`).join('\n')}
    (func $entry (export "f") (result i32) (block (result i32)
      ${Array.from({length:count-1},(_,i)=>`(drop (call $f${i}))`).join('\n')}
      (call $f${count-1}))))`;
}
export function repeated(count) {
  return `(module (func $id (@type.params $T) (param $x $T) (result $T) $x)
    (func $entry (export "f") (result i32) (block (result i32)
      ${Array.from({length:count},()=>'(drop (call $id<i32> 42))').join('\n')} 42)))`;
}
export const programs=[['cache-more-than-256-definitions',functions(300),42],['cache-reuses-repeated-calls',repeated(1100),42]];
export const boundedInvalid=[['emitted-function-pool-limit',functions(1100),150]];
export const invalid=[];
programs.push(['unit-return-in-a-project',`(module (@use "walz:core" (type $unit))
  (func $step (param $ready i32) (block (if (local.get $ready) (then (return)) (else (nop))) (unreachable)))
  (func $entry (export "f") (result i32) (block (result i32) (call $step 1) 42)))`,42]);
invalid.push(['return-with-extra-operands','(module (@use "walz:core" (type $unit)) (func $entry (return (nop) (nop))))',106]);
invalid.push(['unreachable-after-unit-return-is-checked','(module (@use "walz:core" (type $unit)) (func $entry (block (return) (drop (i64.add (i32.const 1) (i64.const 2))))))',110]);

export function logical(op, count, last = '(i32.const 42)') {
  let expression = last;
  for (let i = 0; i < count; i++)
    expression = `(${op} (i32.const ${op === '&&' ? 1 : 0}) ${expression})`;
  return `(module (@use "walz:core" (type $unit))
    (func $entry (export "f") (result i32) ${expression}))`;
}
programs.push(['long-logical-and', logical('&&', 48), 1]);
programs.push(['long-logical-or', logical('||', 48), 1]);
programs.push(['logical-and-short-circuits', logical('&&', 48, '(&& 0 (unreachable))'), 0]);
programs.push(['logical-or-short-circuits', logical('||', 48, '(|| 7 (unreachable))'), 1]);
invalid.push(['logical-checks-skipped-operands', logical('||', 48, '(|| 1 (i64.const 2))'), 110]);
invalid.push(['logical-checks-nested-arity', logical('&&', 48, '(&& 1 2 3)'), 106]);

for (const op of ['&&', '||']) {
  const values = op === '&&' ? [1, 7, 0, 1] : [0, 0, 7, 0];
  const calls = values.map((value, i) => `(call $tick $counter ${value} ${i + 1})`);
  const expression = `(${op} (${op} ${calls[0]} ${calls[1]}) (${op} ${calls[2]} ${calls[3]}))`;
  programs.push([`logical-${op}-source-order`, `(module
    (type $Counter (struct (field $value (mut i32))))
    (func $tick (param $counter (ref $Counter)) (param $value i32) (param $digit i32)
      (result i32) (@effect reads writes)
      (block (result i32)
        (struct.set $Counter $value $counter (+ (* (struct.get $Counter $value $counter) 10) $digit))
        $value))
    (func $entry (export "f") (result i32) (@effect reads writes allocates)
      (@let $counter (struct.new $Counter 0)
        (block (result i32) (drop ${expression}) (struct.get $Counter $value $counter)))))`, 123]);
}
