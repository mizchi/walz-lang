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
function r_n6(p){const n=c.view().getUint8(p);if(n>1)throw new TypeError('invalid bool');return !!n;}
function w_n6(p,v){if(typeof v!=='boolean')throw new TypeError('expected bool');c.view().setUint8(p,v?1:0);}
function r_r7(p){return {"count":r_i32(p+0),"label":r_v12(p+4)};}
function w_r7(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i32(p+0,v["count"]);w_v12(p+4,v["label"]);}
function r_r8(p){return {"id":r_i64(p+0),"name":r_v12(p+8),"values":r_v11(p+16),"payload":r_v9(p+24),"enabled":r_n6(p+32),"weight":r_f64(p+40),"child":r_v13(p+48)};}
function w_r8(p,v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');w_i64(p+0,v["id"]);w_v12(p+8,v["name"]);w_v11(p+16,v["values"]);w_v9(p+24,v["payload"]);w_n6(p+32,v["enabled"]);w_f64(p+40,v["weight"]);w_v13(p+48,v["child"]);}
function r_v9(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return bytes;}
function w_v9(p,v){if(!(v instanceof Uint8Array))throw new TypeError('expected Uint8Array');const bytes=v;const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v10(p){const tag=c.view().getUint8(p);if(tag===0)return {tag:'ok',val:r_r8(p+8)};if(tag===1)return {tag:'err',val:r_i32(p+8)};throw new TypeError('invalid result');}
function w_v10(p,v){if(v?.tag==='ok'){c.view().setUint8(p,0);w_r8(p+8,v.val);}else if(v?.tag==='err'){c.view().setUint8(p,1);w_i32(p+8,v.val);}else throw new TypeError('expected result');}
function r_v11(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>4194304||q%4||q+n*4>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new Int32Array(new Int32Array(c.memory.buffer,q,n));const out=new Int32Array(n);for(let i=0;i<n;i++)out[i]=r_i32(q+i*4);return out;}
function w_v11(p,v){if(!(Array.isArray(v)||v instanceof Int32Array)||v.length>4194304)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*4):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='Int32Array'&&__walzLen.call(v)===n){if(n)new Int32Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_i32(q+i*4,v[i]);}}
function r_v12(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);}
function w_v12(p,v){if(typeof v!=='string')throw new TypeError('expected string');const bytes=new TextEncoder().encode(v);const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v13(p){const tag=c.view().getUint8(p);if(tag===0)return undefined;if(tag!==1)throw new TypeError('invalid option');return r_r7(p+4);}
function w_v13(p,v){if(v===undefined)c.view().setUint8(p,0);else{c.view().setUint8(p,1);w_r7(p+4,v);}}
const api=Object.create(null);
const asyncExports=[];
api["encode"]=(...v)=>c.call('__export_0',1, 64,p=>{w_r8(p+0,v[0]);},p=>r_v9(p),"__post_0",false,v.length);
api["decode"]=(...v)=>c.call('__export_1',1, 8,p=>{w_v9(p+0,v[0]);},p=>r_v10(p),"__post_1",false,v.length);
api["main"]=(...v)=>c.call('__export_2',0, 0,p=>{},p=>r_i32(p),"__post_2",false,v.length);
return {api,asyncExports};
}
