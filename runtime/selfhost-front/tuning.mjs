// Tuning inputs v1: closed generic list specializations over nominal GC types.
export function genericLists(count) {
  return `(module (@use "walz:core" (type $list))
    ${Array.from({length:count},(_,i)=>`(type $S${i} (struct (field $value i32)))`).join('\n')}
    (func $entry (export "f") (result i32)
      (block (result i32)
        ${Array.from({length:count},(_,i)=>`(drop (@let $xs (@list.new (ref $S${i}) (struct.new $S${i} 42))
          (struct.get $S${i} $value (@list.get (ref $S${i}) $xs 0))))`).join('\n')}
        42)))`;
}
export const programs=[['syntax-many-generic-lists',genericLists(16),42]];
