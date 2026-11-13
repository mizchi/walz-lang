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
function r_r0(p){return {"count":r_i32(p+0),"sum":r_i64(p+8),"indices":r_v8(p+16)};}
function w_r0(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i32(p+0,v["count"]);w_i64(p+8,v["sum"]);w_v8(p+16,v["indices"]);}
function r_v8(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>4194304||q%4||q+n*4>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new Int32Array(new Int32Array(c.memory.buffer,q,n));const out=new Int32Array(n);for(let i=0;i<n;i++)out[i]=r_i32(q+i*4);return out;}
function w_v8(p,v){if(!(Array.isArray(v)||v instanceof Int32Array)||v.length>4194304)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*4):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='Int32Array'&&__walzLen.call(v)===n){if(n)new Int32Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_i32(q+i*4,v[i]);}}
const api=Object.create(null);
const asyncExports=[];
api["combine"]=(...v)=>c.call('__export_0',4, 24,p=>{w_i32(p+0,v[0]);w_v8(p+4,v[1]);w_v8(p+12,v[2]);w_i32(p+20,v[3]);},p=>r_v8(p),"__post_0",false,v.length);
api["complement"]=(...v)=>c.call('__export_1',2, 12,p=>{w_i32(p+0,v[0]);w_v8(p+4,v[1]);},p=>r_v8(p),"__post_1",false,v.length);
api["count"]=(...v)=>c.call('__export_2',2, 12,p=>{w_i32(p+0,v[0]);w_v8(p+4,v[1]);},p=>r_i32(p),"__post_2",false,v.length);
api["intersectionCount"]=(...v)=>c.call('__export_3',3, 20,p=>{w_i32(p+0,v[0]);w_v8(p+4,v[1]);w_v8(p+12,v[2]);},p=>r_i32(p),"__post_3",false,v.length);
api["indices"]=(...v)=>c.call('__export_4',2, 12,p=>{w_i32(p+0,v[0]);w_v8(p+4,v[1]);},p=>r_v8(p),"__post_4",false,v.length);
api["roundtrip"]=(...v)=>c.call('__export_5',1, 8,p=>{w_v8(p+0,v[0]);},p=>r_v8(p),"__post_5",false,v.length);
api["get"]=(...v)=>c.call('__export_6',2, 12,p=>{w_v8(p+0,v[0]);w_i32(p+8,v[1]);},p=>r_i32(p),"__post_6",false,v.length);
api["sum"]=(...v)=>c.call('__export_7',1, 8,p=>{w_v8(p+0,v[0]);},p=>r_i64(p),"__post_7",false,v.length);
api["sumWhere"]=(...v)=>c.call('__export_8',2, 16,p=>{w_v8(p+0,v[0]);w_v8(p+8,v[1]);},p=>r_i64(p),"__post_8",false,v.length);
api["equal"]=(...v)=>c.call('__export_9',2, 12,p=>{w_v8(p+0,v[0]);w_i32(p+8,v[1]);},p=>r_v8(p),"__post_9",false,v.length);
api["between"]=(...v)=>c.call('__export_10',3, 16,p=>{w_v8(p+0,v[0]);w_i32(p+8,v[1]);w_i32(p+12,v[2]);},p=>r_v8(p),"__post_10",false,v.length);
api["countEq"]=(...v)=>c.call('__export_11',2, 12,p=>{w_v8(p+0,v[0]);w_i32(p+8,v[1]);},p=>r_i32(p),"__post_11",false,v.length);
api["countBetween"]=(...v)=>c.call('__export_12',3, 16,p=>{w_v8(p+0,v[0]);w_i32(p+8,v[1]);w_i32(p+12,v[2]);},p=>r_i32(p),"__post_12",false,v.length);
api["countMany"]=(...v)=>c.call('__export_13',3, 24,p=>{w_v8(p+0,v[0]);w_v8(p+8,v[1]);w_v8(p+16,v[2]);},p=>r_v8(p),"__post_13",false,v.length);
api["query"]=(...v)=>c.call('__export_14',5, 28,p=>{w_v8(p+0,v[0]);w_v8(p+8,v[1]);w_i32(p+16,v[2]);w_i32(p+20,v[3]);w_i32(p+24,v[4]);},p=>r_r0(p),"__post_14",false,v.length);
return {api,asyncExports};
}
