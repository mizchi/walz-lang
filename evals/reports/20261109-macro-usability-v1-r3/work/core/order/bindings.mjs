// Generated typed boundary functions; GC references stay inside Wasm.
export function bind(c) {
function r_i32(p){return c.view().getInt32(p,true);}
function w_i32(p,v){if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');c.view().setInt32(p,v,true);}
const api=Object.create(null);
const asyncExports=[];
api["main"]=(...v)=>c.call('__export_1',0, 0,p=>{},p=>r_i32(p),"__post_1",false,v.length);
return {api,asyncExports};
}
