// Audit-only interception of real exported calls. Never inspect GC fields or
// implement compiler semantics. Histories contain WeakRefs and scalar counters.
export function installObserver(){
  const native=WebAssembly.instantiate;
  let refs=[],compilerRefs=[],functions={},pinned=null,controlRefs=[],trapNext=false;
  const objectMethods=new Set(['buffer-new','buffer-finish','files-new','compile','expand','output-value','imports','imports-path']);
  function wrap(name,fn){
    return function(...args){
      if(name==='compile')globalThis.__selfhostAuditSignal?.('start');
      try{
        if(name==='compile'&&trapNext){trapNext=false;throw trap();}
        const value=fn(...args);
        if(value!==null&&typeof value==='object')refs.push({kind:name,ref:new WeakRef(value)});
        return value;
      }finally{if(name==='compile')globalThis.__selfhostAuditSignal?.('done');}
    };
  }
  WebAssembly.instantiate=async function(...args){
    const value=await native.apply(WebAssembly,args),instance=value.instance??value,e=instance.exports;
    if(typeof e?.['files-new']!=='function'||typeof e?.['buffer-new']!=='function'||typeof e?.compile!=='function')return value;
    compilerRefs.push(new WeakRef(e.compile));
    functions=Object.fromEntries(Object.entries(e).filter(([,fn])=>typeof fn==='function').map(([name,fn])=>[name,new WeakRef(fn)]));
    const exports=Object.fromEntries(Object.entries(e).map(([name,fn])=>[name,objectMethods.has(name)?wrap(name,fn):fn]));
    return value.instance?{...value,instance:{exports}}:{exports};
  };
  const call=(name,...args)=>{
    const fn=functions[name]?.deref();if(!fn)throw new Error('compiler export is no longer alive: '+name);return fn(...args);
  };
  function bytes(data){
    const buffer=call('buffer-new',data.length);
    for(const byte of data)call('buffer-push',buffer,byte);
    return call('buffer-finish',buffer);
  }
  function trap(){
    const value=bytes(Uint8Array.of(1,2,3));refs.push({kind:'trapped-bytes',ref:new WeakRef(value)});
    try{call('bytes-at',value,-1);}catch(error){return error;}
    throw new Error('expected a real Wasm bounds trap');
  }
  return {
    drain(){
      const observed=refs.length,live={},remaining=[];
      for(const row of refs)if(row.ref.deref()!==undefined){live[row.kind]=(live[row.kind]??0)+1;remaining.push(row);}
      refs=remaining;return {observed,live};
    },
    compilerAlive(){return compilerRefs.filter(ref=>ref.deref()!==undefined).length;},
    pin(size=32768){
      if(pinned)throw new Error('control already pinned');
      const holder=call('files-new'),path=bytes(new TextEncoder().encode('memory.walz'));
      const source=bytes(new Uint8Array(size).fill(120));
      call('files-push',holder,path,source);pinned=holder;
      controlRefs=[new WeakRef(path),new WeakRef(source)];return controlRefs.length;
    },
    controlAlive(){return controlRefs.filter(ref=>ref.deref()!==undefined).length;},
    unpin(){pinned=null;},
    trap,
    failNextCompile(){trapNext=true;},
    restore(){WebAssembly.instantiate=native;},
  };
}
