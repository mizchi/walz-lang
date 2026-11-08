// Driver state stays in this agent. The wire protocol carries cloneable data,
// compiled modules and shared memories, never Wasm GC refs or closures.
const node=typeof process!=='undefined'&&!!process.versions?.node;
const port=node?(await import('node:worker_threads')).parentPort:globalThis;
const send=(message,transfer=[])=>port.postMessage(message,transfer);
let driver,initializing,current,workerId,cancelFlag;
const cancelled=()=>cancelFlag&&(Atomics.load(cancelFlag,0)&(1<<workerId))!==0;
async function receive(m){
  try{
    if(m.type==='init'){
      if(initializing)throw new Error('worker is already initialized');
      workerId=m.workerId;
      if(m.cancellation)cancelFlag=new Int32Array(m.cancellation.memory.buffer,m.cancellation.byteOffset,1);
      initializing=(async()=>{
        const factory=await import(m.driver);
        driver=await factory.create(m.init,{workerId});
        if(typeof driver?.run!=='function')throw new TypeError('driver.create must return {run, dispose?}');
      })();
      await initializing;send({id:m.id,type:'ready'});
    }else if(m.type==='cancel'){
      if(current?.batch===m.batch)current.controller.abort(new DOMException('Aborted','AbortError'));
    }else if(m.type==='run'){
      await initializing;
      if(current)throw new Error('worker already has a running job');
      const controller=new AbortController();current={batch:m.batch,controller};
      const context={workerId,signal:controller.signal,isCancelled:()=>controller.signal.aborted||!!cancelled()};
      if(context.isCancelled())controller.abort(new DOMException('Aborted','AbortError'));
      try{
        const result=await driver.run(m.payload,context);
        if(!result||!Object.hasOwn(result,'value'))throw new TypeError('driver.run must return {value, transfer?}');
        send({id:m.id,type:'result',value:result.value},result.transfer??[]);
      }finally{current=null;}
    }else if(m.type==='close'){
      try{await initializing;}catch{}
      if(current)throw new Error('join running jobs before closing');
      await driver?.dispose?.();driver=null;cancelFlag=null;
      send({id:m.id,type:'closed'});
      if(node)port.close();else globalThis.close();
    }else throw new Error('unknown worker protocol message');
  }catch(error){send({id:m.id,type:'error',message:String(error?.stack??error)});}
}
if(node)port.on('message',receive);else globalThis.onmessage=e=>receive(e.data);
