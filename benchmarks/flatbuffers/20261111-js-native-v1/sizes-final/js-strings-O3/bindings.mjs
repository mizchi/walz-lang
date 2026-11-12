// Experimental generated direct GC boundary.
export function bind(x,c){
function r_i32(v){return v;}
function w_i32(v){if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');return v;}
function r_i64(v){return v;}
function w_i64(v){if(typeof v!=='bigint'||v < -9223372036854775808n||v > 9223372036854775807n)throw new TypeError('expected s64');return v;}
function r_f64(v){return v;}
function w_f64(v){if(typeof v!=='number')throw new TypeError('expected float');return v;}
function r_n7(v){if(v!==0&&v!==1)throw new TypeError('invalid bool');return !!v;}
function w_n7(v){if(typeof v!=='boolean')throw new TypeError('expected bool');return v?1:0;}
function r_r8(v){return {["count"]:r_i32(x.get_r8_0(v)),["label"]:r_v13(x.get_r8_1(v))};}
function w_r8(v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');return x.make_r8(w_i32(v["count"]),w_v13(v["label"]));}
function r_r9(v){return {["id"]:r_i64(x.get_r9_0(v)),["name"]:r_v13(x.get_r9_1(v)),["values"]:r_v12(x.get_r9_2(v)),["payload"]:r_v10(x.get_r9_3(v)),["enabled"]:r_n7(x.get_r9_4(v)),["weight"]:r_f64(x.get_r9_5(v)),["child"]:r_v14(x.get_r9_6(v))};}
function w_r9(v){if(v===null||typeof v!=='object'||Array.isArray(v))throw new TypeError('expected record');return x.make_r9(w_i64(v["id"]),w_v13(v["name"]),w_v12(v["values"]),w_v10(v["payload"]),w_n7(v["enabled"]),w_f64(v["weight"]),w_v14(v["child"]));}
function r_v10(v){return c.readBytes(x.get_v10(v));}
function w_v10(v){return x.make_v10(c.writeBytes(v));}
function r_v11(v){return x.tag_v11(v)===0?{tag:'ok',val:r_r9(x.get_v11_0_0(v))}:{tag:'err',val:r_i32(x.get_v11_1_0(v))};}
function w_v11(v){if(v?.tag==='ok'){return x.make_v11_0(w_r9(v.val));}if(v?.tag==='err'){return x.make_v11_1(w_i32(v.val));}throw new TypeError('expected result');}
function r_v12(v){return c.readList(x.get_v12(v),Int32Array,4,'Int32');}
function w_v12(v){const n=c.writeList(v,Int32Array,'Int32Array',4,'Int32',w_i32);return x.make_v12(n);}
function r_v13(v){return x.get_v13(v);}
function w_v13(v){if(typeof v!=='string')throw new TypeError('expected string');return x.make_v13(c.wellFormed(v));}
function r_v14(v){return x.tag_v14(v)===0?undefined:r_r8(x.get_v14_1_0(v));}
function w_v14(v){if(v===undefined)return x.make_v14_0();return x.make_v14_1(w_r8(v));}

const api=Object.create(null);
api["encode"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_r9(v[0]);return r_v10(c.invoke(()=>x.__call_0(a0)));});
api["decode"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v10(v[0]);return r_v11(c.invoke(()=>x.__call_1(a0)));});
api["main"]=(...v)=>c.run(()=>{if(v.length!==0)throw new TypeError('argument count');return r_i32(c.invoke(()=>x.__call_2()));});
return api;
}
