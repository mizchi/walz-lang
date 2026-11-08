// Bounded by-value channels; accepted counts include buffered and delivered items.
export function createChannels() {
  const entries=new WeakMap(),active=new Set();
  function entry(handle,kind){const s=entries.get(handle);if(!s||kind&&s.kind!==kind)throw new TypeError('invalid channel');return s;}
  function release(s){if(s.writerClosed&&s.readerClosed)active.delete(s);}
  const buffered=s=>s.queue.length-s.head;
  function finishWrite(s,code){const w=s.write;s.write=null;if(w){w.items.length=0;w.resolve([w.accepted,code]);}}
  function takeQueue(s,max){const end=Math.min(s.queue.length,s.head+max),items=s.queue.slice(s.head,end);s.queue.fill(undefined,s.head,end);s.head=end;if(s.head===s.queue.length){s.queue.length=0;s.head=0;}else if(s.head>=1024){s.queue=s.queue.slice(s.head);s.head=0;}return items;}
  function fill(s){
    const w=s.write;if(!w)return;
    while(w.index<w.items.length&&buffered(s)<s.capacity){s.queue.push(w.items[w.index]);w.items[w.index++]=undefined;w.accepted++;}
    if(w.index===w.items.length)finishWrite(s,0);
  }
  function endReader(s){
    if(s.readerClosed)return;
    s.readerClosed=true;s.queue.length=0;s.head=0;
    if(s.kind==='future'){s.completion?.resolve({tag:'err',val:1});s.completion=null;}
    else{finishWrite(s,1);const read=s.read;s.read=null;read?.resolve({items:[],done:true});}
    release(s);
  }
  function pump(s){
    if(s.read&&(buffered(s)||s.write||s.writerClosed)){
      const read=s.read;s.read=null;
      const items=takeQueue(s,read.max),w=s.write;
      while(w&&items.length<read.max&&w.index<w.items.length){items.push(w.items[w.index]);w.items[w.index++]=undefined;w.accepted++;}
      if(w&&w.index===w.items.length)finishWrite(s,0);
      fill(s);
      const done=s.writerClosed&&!buffered(s)&&!s.write;
      if(done)endReader(s);
      read.resolve({items,done});
    }else fill(s);
  }
  function make(kind,capacity){const s={kind,capacity,queue:[],head:0,writerClosed:false,readerClosed:false,taken:false,read:null,write:null};if(kind==='future')s.completion=Promise.withResolvers();const handle=Object.freeze({});entries.set(handle,s);active.add(s);return handle;}
  function close(handle){const s=entry(handle);if(s.writerClosed)return;s.writerClosed=true;if(s.kind==='future'||!s.taken)endReader(s);else{finishWrite(s,1);pump(s);if(!buffered(s))endReader(s);}s.completion=null;release(s);}
  function sendMany(handle,values){
    const s=entry(handle,'stream');
    if((!Array.isArray(values)&&!ArrayBuffer.isView(values))||!Number.isInteger(values.length)||values.length>65536)throw new TypeError('expected bounded batch');
    if(s.writerClosed||s.readerClosed)return Promise.resolve([0,1]);
    if(s.write)throw new TypeError('concurrent stream write');
    if(values.length===0)return Promise.resolve([0,0]);
    const w={...Promise.withResolvers(),items:Array.from(values),index:0,accepted:0};s.write=w;
    w.promise[Symbol.asyncDispose]=()=>{if(s.write===w)finishWrite(s,1);};pump(s);
    return w.promise;
  }
  return {
    stream(capacity){if(!Number.isInteger(capacity)||capacity<0||capacity>65536)throw new RangeError('invalid channel capacity');return make('stream',capacity);},
    future:()=>make('future',1),
    reader(handle){
      const s=entry(handle);if(s.taken)throw new TypeError('channel reader already taken');s.taken=true;
      if(s.kind==='future'){const promise=s.completion?.promise??Promise.resolve({tag:'err',val:1});promise[Symbol.asyncDispose]=()=>endReader(s);return promise;}
      return {
        [Symbol.asyncIterator](){return this;},
        readMany(max){
          if(!Number.isInteger(max)||max<1||max>65536)throw new RangeError('invalid batch count');
          if(s.read)throw new TypeError('concurrent stream read');
          if(s.readerClosed)return Promise.resolve({items:[],done:true});
          const read={...Promise.withResolvers(),max};s.read=read;pump(s);return read.promise;
        },
        next(){return this.readMany(1).then(({items})=>items.length?{done:false,value:items[0]}:{done:true});},
        return(){endReader(s);return Promise.resolve({done:true});},
        [Symbol.asyncDispose](){return this.return();},
      };
    },
    sendMany,
    send(handle,value){const write=sendMany(handle,[value]);const promise=write.then(([,status])=>status);promise[Symbol.asyncDispose]=()=>write[Symbol.asyncDispose]?.();return promise;},
    resolve(handle,value){const s=entry(handle,'future');if(s.writerClosed||s.readerClosed)return 1;s.completion.resolve({tag:'ok',val:value});s.completion={promise:s.completion.promise};s.readerClosed=true;release(s);return 0;},
    close,
    closeAll(){for(const s of active){s.writerClosed=true;endReader(s);s.completion=null;release(s);}},
    active:()=>active.size,
  };
}
