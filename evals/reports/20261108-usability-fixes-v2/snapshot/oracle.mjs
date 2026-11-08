import {mkdir} from 'node:fs/promises';
import {join} from 'node:path';
const minimum=-(1n<<63n), maximum=(1n<<63n)-1n;
export function expenseOracle(lines,nonnegative=false){
 const totals=new Map();let accepted=0n,rejected=0n;
 for(const line of lines){
  let row;
  try{row=JSON.parse(line,(key,value,context)=>key==='amount'&&typeof value==='number'&&/^-?(0|[1-9][0-9]*)$/.test(context.source)?BigInt(context.source):value);}catch{rejected++;continue;}
  if(!row||typeof row.category!=='string'||typeof row.amount!=='bigint'||row.amount<minimum||row.amount>maximum||(nonnegative&&row.amount<0n)){rejected++;continue;}
  const sum=(totals.get(row.category)??0n)+row.amount;
  if(sum<minimum||sum>maximum){rejected++;continue;}
  totals.set(row.category,sum);accepted++;
 }
 return {accepted,rejected,totals:Object.fromEntries(totals)};
}
export function parseReport(text){
 return JSON.parse(text,(_key,value,context)=>typeof value==='number'?BigInt(context.source):value);
}
export async function createRunDirectory(root,id){
 if(!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(id))throw new Error('expected a simple, nonempty evaluation ID');
 await mkdir(root,{recursive:true});const path=join(root,id);await mkdir(path);return path;
}
