// Experimental generated direct GC boundary.
export function bind(x,c){
function r_i32(v){return v;}
function w_i32(v){if(!Number.isInteger(v)||v < -2147483648||v > 2147483647)throw new TypeError('expected s32');return v;}
function r_i64(v){return v;}
function w_i64(v){if(typeof v!=='bigint'||v < -9223372036854775808n||v > 9223372036854775807n)throw new TypeError('expected s64');return v;}
function r_f32(v){return v;}
function w_f32(v){if(typeof v!=='number')throw new TypeError('expected float');return v;}
function r_v1(v){return c.readBytes(x.get_v1(v));}
function w_v1(v){return x.make_v1(c.writeBytes(v));}
function r_v2(v){return c.decode(x.get_v2(v));}
function w_v2(v){return x.make_v2(c.writeText(v));}
function r_v3(v){return x.tag_v3(v)===0?undefined:r_v2(x.get_v3_1_0(v));}
function w_v3(v){if(v===undefined)return x.make_v3_0();return x.make_v3_1(w_v2(v));}
function r_v4(v){return c.readList(x.get_v4(v),BigInt64Array,8,'BigInt64');}
function w_v4(v){const n=c.writeList(v,BigInt64Array,'BigInt64Array',8,'BigInt64',w_i64);return x.make_v4(n);}
function r_v5(v){return c.readList(x.get_v5(v),Float32Array,4,'Float32');}
function w_v5(v){const n=c.writeList(v,Float32Array,'Float32Array',4,'Float32',w_f32);return x.make_v5(n);}
function r_v6(v){return c.readList(x.get_v6(v),Int32Array,4,'Int32');}
function w_v6(v){const n=c.writeList(v,Int32Array,'Int32Array',4,'Int32',w_i32);return x.make_v6(n);}
function r_v7(v){const n=x.len_v7(v);if(n>2097152)throw new RangeError('list limit');return Array.from({length:n},(_,i)=>r_v6(x.get_v7(v,i)));}
function w_v7(v){const n=c.arrayLength(v,2097152);if(!n)return x.empty_v7();const a=x.make_v7(w_v6(v[0]),n);for(let i=1;i<n;i++)x.set_v7(a,i,w_v6(v[i]));return a;}
function r_v8(v){return x.tag_v8(v)===0?{tag:'none'}:{tag:'some',val:r_v3(x.get_v8_1_0(v))};}
function w_v8(v){if(v?.tag!=='none'&&v?.tag!=='some')throw new TypeError('expected option');if(v.tag==='none')return x.make_v8_0();return x.make_v8_1(w_v3(v.val));}
function r_v9(v){return x.tag_v9(v)===0?undefined:r_unit();}
function w_v9(v){if(v===undefined)return x.make_v9_0();w_unit(v);return x.make_v9_1();}
function r_v10(v){x.tag_v10(v);return [r_unit(),r_v9(x.get_v10_0_0(v))];}
function w_v10(v){if(!Array.isArray(v)||v.length!==2)throw new TypeError('expected pair');w_unit(v[0]);return x.make_v10_0(w_v9(v[1]));}
function r_v11(v){const n=x.len_v11(v);if(n>16777216)throw new RangeError('list limit');return Array.from({length:n},()=>r_unit());}
function w_v11(v){const n=c.arrayLength(v,16777216);for(let i=0;i<n;i++)w_unit(v[i]);return x.make_v11(n);}
function r_unit(v){return 'value';}
function w_unit(v){if(v!=='value')throw new TypeError('expected unit');}

const api=Object.create(null);
api["echo"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v2(v[0]);return r_v2(c.invoke(()=>x.__call_0(a0)));});
api["length"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v2(v[0]);return r_i32(c.invoke(()=>x.__call_1(a0)));});
api["lengthCatch"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v2(v[0]);return r_i32(c.invoke(()=>x.__call_2(a0)));});
api["concat"]=(...v)=>c.run(()=>{if(v.length!==2)throw new TypeError('argument count');const a0=w_v2(v[0]);const a1=w_v2(v[1]);return r_v2(c.invoke(()=>x.__call_3(a0,a1)));});
api["literal"]=(...v)=>c.run(()=>{if(v.length!==0)throw new TypeError('argument count');return r_v2(c.invoke(()=>x.__call_4()));});
api["fromBytes"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v1(v[0]);return r_v3(c.invoke(()=>x.__call_5(a0)));});
api["bytes"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v1(v[0]);return r_v1(c.invoke(()=>x.__call_6(a0)));});
api["longs"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v4(v[0]);return r_v4(c.invoke(()=>x.__call_7(a0)));});
api["floats"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v5(v[0]);return r_v5(c.invoke(()=>x.__call_8(a0)));});
api["nested"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v7(v[0]);return r_v7(c.invoke(()=>x.__call_9(a0)));});
api["options"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v8(v[0]);return r_v8(c.invoke(()=>x.__call_10(a0)));});
api["units"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v10(v[0]);return r_v10(c.invoke(()=>x.__call_11(a0)));});
api["unitList"]=(...v)=>c.run(()=>{if(v.length!==1)throw new TypeError('argument count');const a0=w_v11(v[0]);return r_v11(c.invoke(()=>x.__call_12(a0)));});
api["trap"]=(...v)=>c.run(()=>{if(v.length!==0)throw new TypeError('argument count');c.invoke(()=>x.__call_13());return undefined;});
return api;
}
