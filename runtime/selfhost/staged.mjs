// Core-only browser/Node bridge. JS never reads GC fields or implements
// compiler semantics; values stay opaque and are consumed by their own instance.
const encoder=new TextEncoder(), decoder=new TextDecoder('utf-8',{fatal:true});
function require(condition,message){if(!condition)throw new Error(message);}
// V8's lazy Error stack can retain this bridge's closures and the whole guest
// instance. Resolve stacks at the public boundary, preserving error identity.
function materializeError(error,seen=new Set()){
  if(!(error instanceof Error)||seen.has(error))return;
  seen.add(error);
  try{void error.stack;}catch{/* A custom getter must not replace the failure. */}
  materializeError(Object.getOwnPropertyDescriptor(error,'cause')?.value,seen);
  const errors=Object.getOwnPropertyDescriptor(error,'errors')?.value;
  if(Array.isArray(errors))for(const nested of errors)materializeError(nested,seen);
}
const boundary=operation=>(...args)=>{
  try{return operation(...args);}catch(error){materializeError(error);throw error;}
};
export async function openStage(binary){
  const module=await WebAssembly.compile(binary);
  require(WebAssembly.Module.imports(module).length===0,'a compiler stage must have no host imports');
  const {exports:e}=await WebAssembly.instantiate(module);
  for(const name of ['buffer-new','buffer-push','buffer-finish','bytes-length','bytes-at',
    'files-new','files-push','compile','output-ok','output-value','output-error',
    'imports','imports-ok','imports-length','imports-path','imports-error'])
    require(typeof e[name]==='function','missing stage export '+name);
  function lower(bytes){
    require(bytes instanceof Uint8Array,'source must be bytes');
    const buffer=e['buffer-new'](bytes.length);
    for(const byte of bytes)e['buffer-push'](buffer,byte);
    return e['buffer-finish'](buffer);
  }
  function lift(value){
    const bytes=new Uint8Array(e['bytes-length'](value));
    for(let i=0;i<bytes.length;i++)bytes[i]=e['bytes-at'](value,i);
    return bytes;
  }
  const error=(value,get)=>Object.fromEntries(['code','file','start','end'].map((name,i)=>[name,get(value,i)]));
  function project(method,files){
    require(typeof e[method]==='function','missing stage export '+method);
    require(Array.isArray(files),'files must be an array');
    const input=e['files-new']();
    for(const {path,source} of files){
      require(typeof path==='string','path must be a string');
      require(e['files-push'](input,lower(encoder.encode(path)),lower(source))===1,'invalid UTF-8 path');
    }
    const value=e[method](input);
    return e['output-ok'](value)
      ? {tag:'ok',val:lift(e['output-value'](value))}
      : {tag:'err',val:error(value,e['output-error'])};
  }
  return {
    imports:boundary(source=>{
      const value=e.imports(lower(source));
      if(!e['imports-ok'](value))return {tag:'err',val:error(value,e['imports-error'])};
      return {tag:'ok',val:Array.from({length:e['imports-length'](value)},(_,i)=>({
        path:decoder.decode(lift(e['imports-path'](value,i))),source:new Uint8Array(),
      }))};
    }),
    compileProject:boundary(files=>project('compile',files)),
    expandProject:boundary(files=>project('expand',files)),
  };
}
