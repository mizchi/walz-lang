// Core-only stage verification bridge. JS never reads GC fields or implements
// compiler semantics; values stay opaque and are consumed by their own instance.
import assert from 'node:assert/strict';
const encoder=new TextEncoder(), decoder=new TextDecoder('utf-8',{fatal:true});
export async function openStage(binary){
  const module=await WebAssembly.compile(binary);
  assert.deepEqual(WebAssembly.Module.imports(module),[],'a compiler stage must have no host imports');
  const {exports:e}=await WebAssembly.instantiate(module);
  for(const name of ['buffer-new','buffer-push','buffer-finish','bytes-length','bytes-at',
    'files-new','files-push','compile','output-ok','output-value','output-error',
    'imports','imports-ok','imports-length','imports-path','imports-error'])
    assert.equal(typeof e[name],'function','missing stage export '+name);
  function lower(bytes){
    assert.ok(bytes instanceof Uint8Array,'source must be bytes');
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
  return {
    imports(source){
      const value=e.imports(lower(source));
      if(!e['imports-ok'](value))return {tag:'err',val:error(value,e['imports-error'])};
      return {tag:'ok',val:Array.from({length:e['imports-length'](value)},(_,i)=>({
        path:decoder.decode(lift(e['imports-path'](value,i))),source:new Uint8Array(),
      }))};
    },
    compileProject(files){
      assert.ok(Array.isArray(files));
      const input=e['files-new']();
      for(const {path,source} of files){
        assert.equal(typeof path,'string');
        assert.equal(e['files-push'](input,lower(encoder.encode(path)),lower(source)),1,'invalid UTF-8 path');
      }
      const value=e.compile(input);
      return e['output-ok'](value)
        ? {tag:'ok',val:lift(e['output-value'](value))}
        : {tag:'err',val:error(value,e['output-error'])};
    },
  };
}
