// Experimental synchronous, shared-value ABI. No Task objects or handle tables.
const limit=16*1024*1024;
const ta=Object.getPrototypeOf(Uint8Array.prototype);
const tag=Object.getOwnPropertyDescriptor(ta,Symbol.toStringTag).get;
const length=Object.getOwnPropertyDescriptor(ta,'length').get;
const le=new Uint8Array(Uint16Array.of(1).buffer)[0]===1;
const wellFormed=String.prototype.toWellFormed;
function materialize(error,seen=new Set()){
  if(!(error instanceof Error)||seen.has(error))return;
  seen.add(error);try{void error.stack;}catch{}
  materialize(Object.getOwnPropertyDescriptor(error,'cause')?.value,seen);
  const errors=Object.getOwnPropertyDescriptor(error,'errors')?.value;
  if(Array.isArray(errors))for(const e of errors)materialize(e,seen);
}
export async function instantiate(base,bind,manifest,options={}){
  if(manifest.abi!=='walz-js-native-experiment-v1')throw new Error('unsupported native ABI');
  const compileOptions=manifest.jsStrings?{builtins:['js-string'],importedStringConstants:'walz:strings'}:{};
  const load=options.getCoreModule??(async name=>{
    const url=new URL(name,base);
    if(url.protocol==='file:')return WebAssembly.compile(await(await import('node:fs/promises')).readFile(url),compileOptions);
    return WebAssembly.compileStreaming(fetch(url),compileOptions);
  });
  const module=await load('native.wasm',compileOptions);
  if(manifest.jsStrings&&WebAssembly.Module.imports(module).some(i=>i.module==='wasm:js-string'||i.module==='walz:strings'))throw new Error('JS string builtins/imported constants are required');
  if(manifest.jsStrings&&typeof wellFormed!=='function')throw new Error('String.toWellFormed is required');
  const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true});
  let memory,busy=false,failure,entered=false,hostFailure;
  const host=fn=>(...args)=>{try{return fn(...args);}catch(error){hostFailure??=error;throw error;}};
  const c={
    get memory(){return memory;},
    reserve(n){
      if(!Number.isInteger(n)||n<0||n>limit)throw new RangeError('transfer limit');
      if(n>memory.buffer.byteLength)memory.grow(Math.ceil(n/65536)-memory.buffer.byteLength/65536);
    },
    writeBytes(v){
      if(tag.call(v)!=='Uint8Array')throw new TypeError('expected Uint8Array');
      const n=length.call(v);c.reserve(n);new Uint8Array(memory.buffer,0,n).set(v);return n;
    },
    writeText(v){if(typeof v!=='string')throw new TypeError('expected string');return c.writeBytes(encoder.encode(v));},
    wellFormed(v){return wellFormed.call(v);},
    readBytes(n){c.reserve(n);return new Uint8Array(memory.buffer,0,n).slice();},
    decode(n){c.reserve(n);return decoder.decode(new Uint8Array(memory.buffer,0,n));},
    arrayLength(v,max){if(!Array.isArray(v))throw new TypeError('expected list');const n=v.length;if(!Number.isInteger(n)||n<0||n>max)throw new RangeError('list limit');return n;},
    writeList(v,ArrayType,name,width,kind,check){
      const array=Array.isArray(v),n=array?c.arrayLength(v,limit/width):length.call(v);
      if(!array&&tag.call(v)!==name)throw new TypeError('expected '+name);
      c.reserve(n*width);
      if(!array&&le)new ArrayType(memory.buffer,0,n).set(v);
      else{const view=new DataView(memory.buffer);for(let i=0;i<n;i++)view['set'+kind](i*width,check(v[i]),true);}
      return n;
    },
    readList(n,ArrayType,width,kind){
      c.reserve(n*width);
      if(le)return new ArrayType(new ArrayType(memory.buffer,0,n));
      const view=new DataView(memory.buffer),out=new ArrayType(n);for(let i=0;i<n;i++)out[i]=view['get'+kind](i*width,true);return out;
    },
    invoke(fn){entered=true;const value=fn();if(hostFailure)throw hostFailure;return value;},
    run(fn){
      if(failure)throw failure;if(busy)throw new Error('native instance is busy');
      busy=true;entered=false;
      try{return fn();}catch(error){
        if(entered){failure=new Error('native instance failed',{cause:error});materialize(failure);}
        throw error;
      }finally{busy=false;}
    },
  };
  const imports=manifest.jsStrings?{__native:{encode:host(v=>c.writeText(v)),decode:host(n=>c.decode(n))}}:{};
  const instance=await(options.instantiateCore??WebAssembly.instantiate)(module,imports);
  memory=instance.exports.__transfer;
  const api=bind(instance.exports,c);
  return {api,__memory:()=>({hostHandles:0,boundaryBuffers:0,scratchCapacityBytes:memory.buffer.byteLength})};
}
