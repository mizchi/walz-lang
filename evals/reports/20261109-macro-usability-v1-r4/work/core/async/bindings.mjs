// Generated typed boundary functions; GC references stay inside Wasm.
export function bind(c) {
function r_i32(p){return c.view().getInt32(p,true);}
function w_i32(p,v){if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');c.view().setInt32(p,v,true);}
function r_i64(p){return c.view().getBigInt64(p,true);}
function w_i64(p,v){if(typeof v!=='bigint'||v < -9223372036854775808n||v > 9223372036854775807n)throw new TypeError('expected s64');c.view().setBigInt64(p,v,true);}
function r_v0(p){return c.take(0,c.view().getUint32(p,true));}
function w_v0(p,v){c.view().setUint32(p,c.add(0,v),true);}
c.endpoint(0,'future',8,p=>r_i64(p),(p,v)=>{w_i64(p,v);});
const api=Object.create(null);
const asyncExports=[];
c.hostImport('host0',"deferI64", false,v=>c.add(0,v),"wlt:host/event-loop@0.1.0",args=>[args[0],args[1]]);
api["main"]=(...v)=>c.call('__export_1',1, 4,p=>{w_i32(p+0,v[0]);},p=>r_i64(p),"__post_1",true,v.length);
asyncExports.push("main");
api["queued"]=(...v)=>c.call('__export_2',0, 0,p=>{},p=>r_i64(p),"__post_2",false,v.length);
return {api,asyncExports};
}
