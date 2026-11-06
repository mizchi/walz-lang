import {createHash} from "node:crypto";
export const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
export function checksum(text) {
  let n=2166136261;
  for(const b of new TextEncoder().encode(text))n=Math.imul(n^b,16777619)>>>0;
  return n|0;
}
export function corpus() {
  const row=i=>({active:i%2===0,id:i,label:`record-${i}-日本語`,values:[i,i+1,i+2]});
  const small={active:true,id:42,name:"sample",values:[1,2,3]};
  const inputs={
    "small-object":JSON.stringify(small),
    "records-128":JSON.stringify(Array.from({length:128},(_,i)=>row(i))),
    "records-512":JSON.stringify(Array.from({length:512},(_,i)=>row(i))),
    "numbers-4096":JSON.stringify(Array.from({length:4096},(_,i)=>(i-2048)/4)),
    "wide-object":JSON.stringify(Object.fromEntries(Array.from({length:2000},(_,i)=>[`k${String(i).padStart(4,"0")}`,i]))),
    "ascii-string":JSON.stringify("x".repeat(48000)),
    "unicode-escapes":'{"text":"'+'\\u65e5\\u672c\\u8a9e\\ud83e\\udd80\\n'.repeat(128)+'"}',
    "nested-64":"[".repeat(64)+'{"x":1}'+"]".repeat(64),
    whitespace:" ".repeat(60000)+JSON.stringify(small),
  };
  return Object.entries(inputs).map(([name,source])=>{
    const value=JSON.parse(source),normalized=JSON.stringify(value);
    return {name,source,bytes:Buffer.byteLength(source),inputHash:hash(source),normalized,
      normalizedHash:checksum(normalized),kind:value===null?0:typeof value==="boolean"?1:typeof value==="number"?2:typeof value==="string"?3:Array.isArray(value)?4:5};
  });
}
