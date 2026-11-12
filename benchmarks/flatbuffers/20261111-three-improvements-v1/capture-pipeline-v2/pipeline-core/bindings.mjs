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
function r_f64(p){return c.view().getFloat64(p,true);}
function w_f64(p,v){if(typeof v!=='number')throw new TypeError('expected f64');c.view().setFloat64(p,v,true);}
function r_n11(p){const n=c.view().getUint8(p);if(n>1)throw new TypeError('invalid bool');return !!n;}
function w_n11(p,v){if(typeof v!=='boolean')throw new TypeError('expected bool');c.view().setUint8(p,v?1:0);}
function r_r0(p){return {"id":r_i64(p+0),"name":r_v18(p+8),"count":r_i32(p+16),"sum":r_i64(p+24),"size":r_i32(p+32),"enabled":r_n11(p+36),"weight":r_f64(p+40),"child":r_v19(p+48)};}
function w_r0(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i64(p+0,v["id"]);w_v18(p+8,v["name"]);w_i32(p+16,v["count"]);w_i64(p+24,v["sum"]);w_i32(p+32,v["size"]);w_n11(p+36,v["enabled"]);w_f64(p+40,v["weight"]);w_v19(p+48,v["child"]);}
function r_r12(p){return {"count":r_i32(p+0),"label":r_v18(p+4)};}
function w_r12(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i32(p+0,v["count"]);w_v18(p+4,v["label"]);}
function r_r13(p){return {"id":r_i64(p+0),"name":r_v18(p+8),"values":r_v27(p+16),"payload":r_v17(p+24),"enabled":r_n11(p+32),"weight":r_f64(p+40),"child":r_v28(p+48)};}
function w_r13(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i64(p+0,v["id"]);w_v18(p+8,v["name"]);w_v27(p+16,v["values"]);w_v17(p+24,v["payload"]);w_n11(p+32,v["enabled"]);w_f64(p+40,v["weight"]);w_v28(p+48,v["child"]);}
function r_v17(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return bytes;}
function w_v17(p,v){if(!(v instanceof Uint8Array))throw new TypeError('expected Uint8Array');const bytes=v;const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v18(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);}
function w_v18(p,v){if(typeof v!=='string')throw new TypeError('expected string');const bytes=new TextEncoder().encode(v);const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v19(p){const tag=c.view().getUint8(p);if(tag===0)return undefined;if(tag!==1)throw new TypeError('invalid option');return r_i32(p+4);}
function w_v19(p,v){if(v===undefined)c.view().setUint8(p,0);else{c.view().setUint8(p,1);w_i32(p+4,v);}}
function r_v20(p){const tag=c.view().getUint8(p);if(tag===0)return {tag:'ok',val:r_r0(p+8)};if(tag===1)return {tag:'err',val:r_i32(p+8)};throw new TypeError('invalid result');}
function w_v20(p,v){if(v?.tag==='ok'){c.view().setUint8(p,0);w_r0(p+8,v.val);}else if(v?.tag==='err'){c.view().setUint8(p,1);w_i32(p+8,v.val);}else throw new TypeError('expected result');}
function r_v21(p){const tag=c.view().getUint8(p);if(tag===0)return {tag:'ok',val:r_v17(p+4)};if(tag===1)return {tag:'err',val:r_i32(p+4)};throw new TypeError('invalid result');}
function w_v21(p,v){if(v?.tag==='ok'){c.view().setUint8(p,0);w_v17(p+4,v.val);}else if(v?.tag==='err'){c.view().setUint8(p,1);w_i32(p+4,v.val);}else throw new TypeError('expected result');}
function r_v22(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>2097152||q%4||q+n*8>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_v17(q+i*8));}
function w_v22(p,v){if(!Array.isArray(v)||v.length>2097152)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*8):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_v17(q+i*8,v[i]);}}
function r_v23(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>262144||q%8||q+n*64>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_v20(q+i*64));}
function w_v23(p,v){if(!Array.isArray(v)||v.length>262144)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*64):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_v20(q+i*64,v[i]);}}
function r_v24(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>1398101||q%4||q+n*12>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_v21(q+i*12));}
function w_v24(p,v){if(!Array.isArray(v)||v.length>1398101)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*12):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_v21(q+i*12,v[i]);}}
function r_v25(p){const tag=c.view().getUint8(p);if(tag===0)return {tag:'ok',val:r_r13(p+8)};if(tag===1)return {tag:'err',val:r_i32(p+8)};throw new TypeError('invalid result');}
function w_v25(p,v){if(v?.tag==='ok'){c.view().setUint8(p,0);w_r13(p+8,v.val);}else if(v?.tag==='err'){c.view().setUint8(p,1);w_i32(p+8,v.val);}else throw new TypeError('expected result');}
function r_v27(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>4194304||q%4||q+n*4>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new Int32Array(new Int32Array(c.memory.buffer,q,n));const out=new Int32Array(n);for(let i=0;i<n;i++)out[i]=r_i32(q+i*4);return out;}
function w_v27(p,v){if(!(Array.isArray(v)||v instanceof Int32Array)||v.length>4194304)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*4):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='Int32Array'&&__walzLen.call(v)===n){if(n)new Int32Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_i32(q+i*4,v[i]);}}
function r_v28(p){const tag=c.view().getUint8(p);if(tag===0)return undefined;if(tag!==1)throw new TypeError('invalid option');return r_r12(p+4);}
function w_v28(p,v){if(v===undefined)c.view().setUint8(p,0);else{c.view().setUint8(p,1);w_r12(p+4,v);}}
const api=Object.create(null);
const asyncExports=[];
api["inspect"]=(...v)=>c.call('__export_0',1, 8,p=>{w_v17(p+0,v[0]);},p=>r_v20(p),"__post_0",false,v.length);
api["transform"]=(...v)=>c.call('__export_2',2, 12,p=>{w_v17(p+0,v[0]);w_i32(p+8,v[1]);},p=>r_v21(p),"__post_2",false,v.length);
api["inspectBatch"]=(...v)=>c.call('__export_5',1, 8,p=>{w_v22(p+0,v[0]);},p=>r_v23(p),"__post_5",false,v.length);
api["transformBatch"]=(...v)=>c.call('__export_6',2, 12,p=>{w_v22(p+0,v[0]);w_i32(p+8,v[1]);},p=>r_v24(p),"__post_6",false,v.length);
api["encode"]=(...v)=>c.call('__export_7',1, 64,p=>{w_r13(p+0,v[0]);},p=>r_v17(p),"__post_7",false,v.length);
api["decode"]=(...v)=>c.call('__export_8',1, 8,p=>{w_v17(p+0,v[0]);},p=>r_v25(p),"__post_8",false,v.length);
return {api,asyncExports};
}
