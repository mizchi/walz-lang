// Value-only adapter over WASI 0.3 descriptor.openAt/readViaStream/writeViaStream.
// Upstream: WebAssembly/WASI v0.3.0, 3ee2a590c766594ae44a54730fc74fc27da5c609.
export const FILESYSTEM = 'wlt:filesystem/files@0.3.0';
const ok = () => ({tag:'ok',val:undefined});
const err = val => ({tag:'err',val});
const codes = new Set(['bad-descriptor','interrupted','invalid','io','no-entry','not-permitted','read-only']);
function errorCode(error) {
  const value=typeof error==='string'?error:error?.val??error?.payload??error?.code;
  if(codes.has(value))return value;
  return ({ENOENT:'no-entry',EACCES:'not-permitted',EPERM:'not-permitted',EBADF:'bad-descriptor',EINVAL:'invalid',EROFS:'read-only',EINTR:'interrupted'})[value]??'io';
}
function unwrap(value) {
  if(value?.tag==='err')throw value.val;
  return value?.tag==='ok'?value.val:value;
}
async function dispose(value) {
  const close=value?.[Symbol.asyncDispose]??value?.[Symbol.dispose];
  if(close)await close.call(value);
}

export function createWasiFileSystem({readFiles={},writeFiles={}}={}) {
  const active=new Set();
  const get=(files,name)=>Object.hasOwn(files,name)?files[name]:undefined;
  const api={
    readFile(name) {
      const entry=get(readFiles,name),complete=Promise.withResolvers(),stop=Promise.withResolvers();
      let started,descriptor,iterator,done=false,settled=false,closing,disposed=false,reading=false;
      const state={close}; active.add(state);
      const settle=value=>{done=true;if(!settled){settled=true;complete.resolve(value);}};
      const closeDescriptor=async()=>{if(descriptor&&!disposed){disposed=true;await dispose(descriptor);}};
      async function close() {
        if(closing)return closing;
        done=true;stop.resolve();
        closing=(async()=>{
          try {await started?.catch(()=>{});try{await closeDescriptor();}finally{await iterator?.return?.();}}
          finally {active.delete(state);settle(err('interrupted'));}
        })();
        return closing;
      }
      const stream={
        [Symbol.asyncIterator](){return this;},
        async readMany(max){
          if(!Number.isInteger(max)||max<1||max>65536)throw new RangeError('invalid batch count');
          if(done)return {items:[],done:true};
          if(reading)throw new TypeError('concurrent file stream read');
          reading=true;
          try {
            // Do not await the completion future before draining its stream.
            let completion;
            if(!started){
              started=(async()=>{
                if(!entry)throw 'not-permitted';
                descriptor=unwrap(await entry.directory.openAt({symlinkFollow:false},entry.path,{}, {read:true}));
                if(done){await closeDescriptor();return;}
                const pair=await descriptor.readViaStream(0n);
                iterator=pair[0][Symbol.asyncIterator]();completion=pair[1];
                state.completion=completion;
                Promise.resolve(completion).catch(()=>{});
              })();
            }
            await started;
            if(done)return {items:[],done:true};
            const result=await Promise.race([iterator.readMany?iterator.readMany(max):Promise.resolve(iterator.next()).then(r=>({items:r.done?[]:[r.value],done:r.done})),stop.promise.then(()=>({items:[],done:true}))]);
            if(done)return {items:[],done:true};
            if(!result?.items||!Number.isInteger(result.items.length)||result.items.length>max||(!result.items.length&&!result.done))throw 'invalid';
            if(result.done){
              const value=await state.completion??ok();
              await closeDescriptor();if(!done)settle(value);active.delete(state);
              return {items:result.items??[],done:true};
            }
            return result;
          } catch(error) {
            settle(err(errorCode(error)));
            try{try{await closeDescriptor();}finally{await iterator?.return?.();}}finally{active.delete(state);}
            return {items:[],done:true};
          }finally{reading=false;}
        },
        async next(){const {items}=await this.readMany(1);return items.length?{done:false,value:items[0]}:{done:true};},
        async return(){await close();return {done:true};},
      };
      complete.promise[Symbol.asyncDispose]=close;
      return [stream,complete.promise];
    },
    writeFile(name,stream) {
      const entry=get(writeFiles,name),stop=Promise.withResolvers();
      const iterator=stream[Symbol.asyncIterator]();
      let cancelled=false,descriptor,operation;
      const state={async close(){cancelled=true;stop.resolve();await operation;}};active.add(state);
      const input={
        [Symbol.asyncIterator](){return this;},
        next(){return cancelled?Promise.resolve({done:true}):Promise.race([iterator.next(),stop.promise.then(()=>({done:true}))]);},
        readMany(max){return cancelled?Promise.resolve({items:[],done:true}):Promise.race([iterator.readMany?iterator.readMany(max):Promise.resolve(iterator.next()).then(r=>({items:r.done?[]:[r.value],done:r.done})),stop.promise.then(()=>({items:[],done:true}))]);},
        return(){return iterator.return?.()??Promise.resolve({done:true});},
      };
      operation=(async()=>{
        try {
          if(!entry)return err('not-permitted');
          descriptor=unwrap(await entry.directory.openAt({symlinkFollow:false},entry.path,{create:true,truncate:true},{write:true}));
          const value=await descriptor.writeViaStream(input,0n);
          return cancelled?err('interrupted'):value??ok();
        } catch(error) {return err(errorCode(error));}
        finally {try{try{await dispose(descriptor);}finally{await iterator.return?.();}}finally{active.delete(state);}}
      })();
      operation[Symbol.asyncDispose]=state.close;
      return operation;
    },
    async close(){const results=await Promise.allSettled([...active].map(s=>s.close()));const failures=results.filter(r=>r.status==='rejected');if(failures.length)throw new AggregateError(failures.map(r=>r.reason),'filesystem cleanup failed');},
    active:()=>active.size,
  };
  return api;
}
