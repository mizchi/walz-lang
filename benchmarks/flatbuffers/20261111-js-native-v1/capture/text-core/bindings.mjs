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
function r_f32(p){return c.view().getFloat32(p,true);}
function w_f32(p,v){if(typeof v!=='number')throw new TypeError('expected f32');c.view().setFloat32(p,v,true);}
function r_v1(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return bytes;}
function w_v1(p,v){if(!(v instanceof Uint8Array))throw new TypeError('expected Uint8Array');const bytes=v;const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v2(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);const bytes=new Uint8Array(c.memory.buffer,q,n).slice();return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);}
function w_v2(p,v){if(typeof v!=='string')throw new TypeError('expected string');const bytes=new TextEncoder().encode(v);const q=bytes.length?c.alloc(bytes.length):0;new Uint8Array(c.memory.buffer,q,bytes.length).set(bytes);c.view().setUint32(p,q,true);c.view().setUint32(p+4,bytes.length,true);}
function r_v3(p){const tag=c.view().getUint8(p);if(tag===0)return undefined;if(tag!==1)throw new TypeError('invalid option');return r_v2(p+4);}
function w_v3(p,v){if(v===undefined)c.view().setUint8(p,0);else{c.view().setUint8(p,1);w_v2(p+4,v);}}
function r_v4(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>2097152||q%8||q+n*8>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new BigInt64Array(new BigInt64Array(c.memory.buffer,q,n));const out=new BigInt64Array(n);for(let i=0;i<n;i++)out[i]=r_i64(q+i*8);return out;}
function w_v4(p,v){if(!(Array.isArray(v)||v instanceof BigInt64Array)||v.length>2097152)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*8):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='BigInt64Array'&&__walzLen.call(v)===n){if(n)new BigInt64Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_i64(q+i*8,v[i]);}}
function r_v5(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>4194304||q%4||q+n*4>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new Float32Array(new Float32Array(c.memory.buffer,q,n));const out=new Float32Array(n);for(let i=0;i<n;i++)out[i]=r_f32(q+i*4);return out;}
function w_v5(p,v){if(!(Array.isArray(v)||v instanceof Float32Array)||v.length>4194304)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*4):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='Float32Array'&&__walzLen.call(v)===n){if(n)new Float32Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_f32(q+i*4,v[i]);}}
function r_v6(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>4194304||q%4||q+n*4>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');if(__walzLE)return new Int32Array(new Int32Array(c.memory.buffer,q,n));const out=new Int32Array(n);for(let i=0;i<n;i++)out[i]=r_i32(q+i*4);return out;}
function w_v6(p,v){if(!(Array.isArray(v)||v instanceof Int32Array)||v.length>4194304)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*4):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);if(__walzLE&&__walzTag.call(v)==='Int32Array'&&__walzLen.call(v)===n){if(n)new Int32Array(c.memory.buffer,q,n).set(v);return;}for(let i=0;i<n;i++){w_i32(q+i*4,v[i]);}}
function r_v7(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>2097152||q%4||q+n*8>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_v6(q+i*8));}
function w_v7(p,v){if(!Array.isArray(v)||v.length>2097152)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*8):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_v6(q+i*8,v[i]);}}
function r_v8(p){const tag=c.view().getUint8(p);if(tag===0)return {tag:'none'};if(tag!==1)throw new TypeError('invalid option');return {tag:'some',val:r_v3(p+4)};}
function w_v8(p,v){if(v?.tag!=='none'&&v?.tag!=='some')throw new TypeError('expected option');if(v?.tag==='none')c.view().setUint8(p,0);else{c.view().setUint8(p,1);w_v3(p+4,v.val);}}
function r_v9(p){const tag=c.view().getUint8(p);if(tag===0)return undefined;if(tag!==1)throw new TypeError('invalid option');return r_unit(p+1);}
function w_v9(p,v){if(v===undefined)c.view().setUint8(p,0);else{c.view().setUint8(p,1);w_unit(p+1,v);}}
function r_v10(p){return [r_unit(p),r_v9(p+1)];}
function w_v10(p,v){if(!Array.isArray(v)||v.length!==2)throw new TypeError('expected pair');w_unit(p,v[0]);w_v9(p+1,v[1]);}
function r_v11(p){const q=c.view().getUint32(p,true),n=c.view().getUint32(p+4,true);if(n>16777216||q%1||q+n*1>c.memory.buffer.byteLength)throw new TypeError('invalid list bounds');return Array.from({length:n},(_,i)=>r_unit(q+i*1));}
function w_v11(p,v){if(!Array.isArray(v)||v.length>16777216)throw new TypeError('expected bounded list');const n=v.length,q=n?c.alloc(n*1):0;c.view().setUint32(p,q,true);c.view().setUint32(p+4,n,true);for(let i=0;i<n;i++){w_unit(q+i*1,v[i]);}}
function r_unit(p){if(c.view().getUint8(p)!==0)throw new TypeError('invalid unit');return 'value';}
function w_unit(p,v){if(v!=='value')throw new TypeError('expected unit');c.view().setUint8(p,0);}
const api=Object.create(null);
const asyncExports=[];
api["echo"]=(...v)=>c.call('__export_0',1, 8,p=>{w_v2(p+0,v[0]);},p=>r_v2(p),"__post_0",false,v.length);
api["length"]=(...v)=>c.call('__export_1',1, 8,p=>{w_v2(p+0,v[0]);},p=>r_i32(p),"__post_1",false,v.length);
api["lengthCatch"]=(...v)=>c.call('__export_2',1, 8,p=>{w_v2(p+0,v[0]);},p=>r_i32(p),"__post_2",false,v.length);
api["concat"]=(...v)=>c.call('__export_3',2, 16,p=>{w_v2(p+0,v[0]);w_v2(p+8,v[1]);},p=>r_v2(p),"__post_3",false,v.length);
api["literal"]=(...v)=>c.call('__export_4',0, 0,p=>{},p=>r_v2(p),"__post_4",false,v.length);
api["fromBytes"]=(...v)=>c.call('__export_5',1, 8,p=>{w_v1(p+0,v[0]);},p=>r_v3(p),"__post_5",false,v.length);
api["bytes"]=(...v)=>c.call('__export_6',1, 8,p=>{w_v1(p+0,v[0]);},p=>r_v1(p),"__post_6",false,v.length);
api["longs"]=(...v)=>c.call('__export_7',1, 8,p=>{w_v4(p+0,v[0]);},p=>r_v4(p),"__post_7",false,v.length);
api["floats"]=(...v)=>c.call('__export_8',1, 8,p=>{w_v5(p+0,v[0]);},p=>r_v5(p),"__post_8",false,v.length);
api["nested"]=(...v)=>c.call('__export_9',1, 8,p=>{w_v7(p+0,v[0]);},p=>r_v7(p),"__post_9",false,v.length);
api["options"]=(...v)=>c.call('__export_10',1, 16,p=>{w_v8(p+0,v[0]);},p=>r_v8(p),"__post_10",false,v.length);
api["units"]=(...v)=>c.call('__export_11',1, 3,p=>{w_v10(p+0,v[0]);},p=>r_v10(p),"__post_11",false,v.length);
api["unitList"]=(...v)=>c.call('__export_12',1, 8,p=>{w_v11(p+0,v[0]);},p=>r_v11(p),"__post_12",false,v.length);
api["trap"]=(...v)=>c.call('__export_13',0, 0,p=>{},p=>undefined,null,false,v.length);
return {api,asyncExports};
}
