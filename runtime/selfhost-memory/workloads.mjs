// Browser-compatible, finite inputs. Every batch cycles the same physical GC
// shapes but uses fresh source identifiers, so an unbounded name cache grows.
export function workload(id){
  const name='$main'+id,base=`(module (func ${name} (export "main") (result i32) (@let $x (type i32) 40 (+ $x 2))))`;
  switch(id%8){
    case 0:return {kind:'scalar',source:base,status:'success',value:42};
    case 1:return {kind:'generic-gc',source:`(module (@use "walz:core" (type $option))
      (func $value${id} (@type.params $T) (param $x $T) (result $T) $x)
      (func ${name} (export "main") (result i64)
        (@let $item (type $option<i64>) (@option.some (call $value${id}<i64> 42))
          (@match $item (@case none () (i64.const 0)) (@case some ($v) $v)))))`,status:'success',value:'42n'};
    case 2:{
      const n=1+(Math.floor(id/8)%16),fields=Array.from({length:n},()=>'(field i32)').join(' ');
      return {kind:'distinct-gc-shape',source:`(module (type $S${id} (struct ${fields}))
        (func ${name} (export "main") (result i32) (struct.get $S${id} 0
          (struct.new $S${id} ${Array.from({length:n},(_,i)=>`(i32.const ${i?i:42})`).join(' ')}))))`,status:'success',value:42};
    }
    case 3:return {kind:'parse-error',source:'(module (func '+name,status:'error'};
    case 4:return {kind:'type-error',source:`(module (func ${name} (result i64) (i32.const 1)))`,status:'error',code:110};
    case 5:return {kind:'unsupported',source:`(module (func ${name} (result f64) (f64.const 42)))`,status:'error'};
    case 6:return {kind:'depth-limit',source:'('.repeat(300)+'module'+')'.repeat(300),status:'error'};
    default:return {kind:'large-source',source:';; '+String(id)+' '+ 'x'.repeat(16*1024)+'\n'+base,status:'success',value:42};
  }
}
export function batches(rounds,calls,start=0){
  return Array.from({length:rounds},(_,round)=>Array.from({length:calls},(_,i)=>workload(start+round*calls+i)));
}
