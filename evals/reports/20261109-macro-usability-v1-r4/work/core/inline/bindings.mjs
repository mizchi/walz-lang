// Generated typed boundary functions; GC references stay inside Wasm.
export function bind(c) {
function r_v20(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);}
function w_v20(p,v){if(typeof v!=='string')throw new TypeError('expected string');const bytes=new TextEncoder().encode(v);const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v36(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>2097152||q%4||q+n*8>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_v20(q+i*8));}
function w_v36(p,v){if(!Array.isArray(v)||v.length>2097152)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*8):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_v20(q+i*8,v[i]);}}
const api=Object.create(null);
const asyncExports=[];
api["summarize"]=(...v)=>c.call('__export_62',1, 8,p=>{w_v36(p+0,v[0]);},p=>r_v20(p),"__post_62",false,v.length);
api["summarizeNonnegative"]=(...v)=>c.call('__export_63',1, 8,p=>{w_v36(p+0,v[0]);},p=>r_v20(p),"__post_63",false,v.length);
api["main"]=(...v)=>c.call('__export_64',0, 0,p=>{},p=>r_v20(p),"__post_64",false,v.length);
return {api,asyncExports};
}
