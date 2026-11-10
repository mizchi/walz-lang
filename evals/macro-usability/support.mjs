import {mkdir} from 'node:fs/promises';
import {join} from 'node:path';

class NumberToken {constructor(source){this.source=source;}}
const min=-(1n<<63n), max=(1n<<63n)-1n;
export function oracle(lines,nonnegative=false){
  const totals=new Map();let accepted=0n,rejected=0n;
  for(const line of lines){
    let value;
    try{value=JSON.parse(line,(_key,v,context)=>typeof v==='number'?new NumberToken(context.source):v);}catch{rejected++;continue;}
    if(!value||typeof value.category!=='string'||!(value.amount instanceof NumberToken)||!/^\-?(0|[1-9][0-9]*)$/.test(value.amount.source)){rejected++;continue;}
    const amount=BigInt(value.amount.source);
    if(amount<min||amount>max||(nonnegative&&amount<0n)){rejected++;continue;}
    const next=(totals.get(value.category)??0n)+amount;
    if(next<min||next>max){rejected++;continue;}
    totals.set(value.category,next);accepted++;
  }
  return {accepted,rejected,totals:Object.fromEntries(totals)};
}
export function parseReport(text){return JSON.parse(text,(_key,v,context)=>typeof v==='number'?BigInt(context.source):v);}
export async function createRunDirectory(root,id){
  if(!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(id))throw new Error('expected a simple nonempty evaluation ID');
  await mkdir(root,{recursive:true});const directory=join(root,id);await mkdir(directory);return directory;
}
export function scalingSource(count,macros){
  if(!Number.isInteger(count)||count<1||count>512)throw new Error('count must be 1..512');
  const header=macros?'(@macro $double (rule (?x:expr) (infer (?T ?x)) (when (Add ?x)) (@let $value (type ?T) ?x (+ $value $value))))\n':'';
  return `(module\n ${header}${Array.from({length:count},(_,i)=>` (func $f${i} (export "f${i}") (param $x i64) (result i64) (@effect pure) ${macros?'(@$double $x)':'(+ $x $x)'})`).join('\n')}\n)`;
}
export const cases=[
  {name:'empty',rows:[]},
  {name:'ordinary and refunds',rows:['{"category":"food","amount":40}','{"category":"food","amount":2}','{"category":"work","amount":-3}']},
  {name:'shape and JSON errors',rows:['null','[]','true','{','{}','{"category":1,"amount":3}','{"category":"x"}','{"category":"x","amount":"3"}','{"category":"x","amount":1.5}','{"category":"x","amount":1e0}']},
  {name:'signed limits and atomic overflow',rows:['{"category":"max","amount":9223372036854775807}','{"category":"max","amount":1}','{"category":"min","amount":-9223372036854775808}','{"category":"min","amount":-1}','{"category":"range","amount":9223372036854775808}','{"category":"range","amount":-9223372036854775809}']},
  {name:'Unicode escaped and special keys',rows:['{"category":"日本😀","amount":7}','{"category":"quote\\\"\\n","amount":-2}','{"category":"__proto__","amount":3}','{"category":"","amount":-0}']},
  {name:'last duplicate field wins',rows:['{"category":"first","category":"last","amount":9,"amount":2}']},
  {name:'large mixed batch',rows:Array.from({length:1500},(_,i)=>i%11===0?'invalid':JSON.stringify({category:`group-${i%29}`,amount:i%101-50}))},
];
