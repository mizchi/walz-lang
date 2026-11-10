export function enumSource(count) {
  const variants=Array.from({length:count},(_,i)=>`(v${i} i32)`).join(' ');
  const arms=Array.from({length:count},(_,i)=>`(case v${i} ($x) (+ $x ${i}))`).reverse().join(' ');
  const makers=Array.from({length:count},(_,i)=>`(func $make${i} (export "make${i}") (param $n i32) (result (ref $E)) (%effect allocates) (%variant.new $E v${i} $n))`).join('\n');
  return `(module (type $E (%variant ${variants}))
  (func $read (export "read") (param $v (ref $E)) (result i32) (%effect pure)
    (%match $E (result i32) $v ${arms}))
  ${makers}
  (func $main (export "main") (result i32) (%effect allocates)
    (call $read (call $make${count-1} (i32.const ${43-count})))))`;
}

export const cases=[
  {name:'scalar',source:'(module (func $main (export "main") (result i32) (%effect pure) (%let $dead (+ (i32.const 1) 2) (i32.const 42))))'},
  {name:'local-struct',source:'(module (type $S (struct (field $n i32))) (func $main (export "main") (result i32) (%effect allocates) (%let $x (struct.new $S 42) (struct.get $S $n $x))))'},
  {name:'local-result',source:'(module (%use "wlt:core" (type $result)) (func $main (export "main") (result i32) (%effect pure) (%let $x (%result.ok i32 i32 42) (%match $x (case ok ($x) $x) (case err ($e) $e)))))'},
  ...[8,16,32].map(count=>({name:`enum-${count}`,count,source:enumSource(count)})),
];
