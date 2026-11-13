// Generated typed boundary functions; GC references stay inside Wasm.
export function bind(c) {
const __walzLE=new Uint8Array(new Uint16Array([1]).buffer)[0]===1;
const __walzTA=Object.getPrototypeOf(Uint8Array.prototype);
const __walzTag=Object.getOwnPropertyDescriptor(__walzTA,Symbol.toStringTag).get;
const __walzLen=Object.getOwnPropertyDescriptor(__walzTA,'length').get;
function r_i32(p){return c.view().getInt32(p,true);}
function w_i32(p,v){if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');c.view().setInt32(p,v,true);}
function r_i64(p){return c.view().getBigInt64(p,true);}
function w_i64(p,v){if(typeof v!=='bigint'||v < -9223372036854775808n||v > 9223372036854775807n)throw new TypeError('expected s64');c.view().setBigInt64(p,v,true);}
function r_r0(p){return {"dictionary":r_v19(p+0),"codes":r_v20(p+8)};}
function w_r0(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_v19(p+0,v["dictionary"]);w_v20(p+8,v["codes"]);}
function r_r1(p){return {"count":r_i32(p+0),"sum":r_i64(p+8),"indices":r_v20(p+16)};}
function w_r1(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i32(p+0,v["count"]);w_i64(p+8,v["sum"]);w_v20(p+16,v["indices"]);}
function r_v18(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);}
function w_v18(p,v){if(typeof v!=='string')throw new TypeError('expected string');const bytes=new TextEncoder().encode(v);const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v19(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>2097152||q%4||q+n*8>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_v18(q+i*8));}
function w_v19(p,v){if(!Array.isArray(v)||v.length>2097152)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*8):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_v18(q+i*8,v[i]);}}
function r_v20(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>4194304||q%4||q+n*4>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new Int32Array(new Int32Array(c.memory.buffer,q,n));const out=new Int32Array(n);for(let i=0;i<n;i++)out[i]=r_i32(q+i*4);return out;}
function w_v20(p,v){if(!(Array.isArray(v)||v instanceof Int32Array)||v.length>4194304)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*4):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='Int32Array'&&__walzLen.call(v)===n){if(n)new Int32Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_i32(q+i*4,v[i]);}}
const api=Object.create(null);
const asyncExports=[];
api["encode"]=(...v)=>c.call('__export_0',1, 8,p=>{w_v19(p+0,v[0]);},p=>r_r0(p),"__post_0",false,v.length);
api["decode"]=(...v)=>c.call('__export_1',1, 16,p=>{w_r0(p+0,v[0]);},p=>r_v19(p),"__post_1",false,v.length);
api["get"]=(...v)=>c.call('__export_2',2, 20,p=>{w_r0(p+0,v[0]);w_i32(p+16,v[1]);},p=>r_v18(p),"__post_2",false,v.length);
api["select"]=(...v)=>c.call('__export_3',2, 16,p=>{w_v19(p+0,v[0]);w_v19(p+8,v[1]);},p=>r_v20(p),"__post_3",false,v.length);
api["equal"]=(...v)=>c.call('__export_4',2, 24,p=>{w_r0(p+0,v[0]);w_v18(p+16,v[1]);},p=>r_v20(p),"__post_4",false,v.length);
api["countMany"]=(...v)=>c.call('__export_5',2, 16,p=>{w_v19(p+0,v[0]);w_v19(p+8,v[1]);},p=>r_v20(p),"__post_5",false,v.length);
api["countEncoded"]=(...v)=>c.call('__export_6',2, 24,p=>{w_r0(p+0,v[0]);w_v19(p+16,v[1]);},p=>r_v20(p),"__post_6",false,v.length);
api["query"]=(...v)=>c.call('__export_7',5, 32,p=>{w_v19(p+0,v[0]);w_v20(p+8,v[1]);w_v19(p+16,v[2]);w_i32(p+24,v[3]);w_i32(p+28,v[4]);},p=>r_r1(p),"__post_7",false,v.length);
return {api,asyncExports};
}
