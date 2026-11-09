// Host grouping of public call scopes. WLT children retain their affine rules.
import {TaskCancelledError,TaskTimeoutError} from './tasks.mjs';
const errorFor=reason=>reason instanceof Error?reason:new TaskCancelledError(reason);
function view(record){return Object.freeze({
  get signal(){return record.controller?.signal;},
  start(name,args=[]){
    if(record.closed)throw new Error('task scope is closed');
    if(record.controller.signal.aborted)throw errorFor(record.controller.signal.reason);
    const task=record.start(name,args,{signal:record.controller.signal});record.active.add(task);
    task.result.then(()=>record.active.delete(task),error=>{
      record.active.delete(task);
      if(task.state==='failed')record.failure??=error;
      if(!record.controller?.signal.aborted)void record.cancel(error);
    });
    return task;
  },
  cancel(reason){return record.cancel?.(reason)??Promise.resolve();},
});}
export async function withTaskScope(start,body,{signal,timeoutMs}={}){
  if(typeof start!=='function'||typeof body!=='function')throw new TypeError('expected start and scope callback');
  if(timeoutMs!==undefined&&(!Number.isInteger(timeoutMs)||timeoutMs<0||timeoutMs>2147483647))throw new RangeError('invalid timeoutMs');
  if(signal&&typeof signal.addEventListener!=='function')throw new TypeError('expected AbortSignal');
  const record={start,controller:new AbortController(),active:new Set(),closed:false};
  const cancel=reason=>{
    if(!record.controller)return Promise.resolve();
    if(!record.controller.signal.aborted)record.controller.abort(reason);
    return Promise.all([...record.active].map(t=>t.cancel(reason)));
  };
  record.cancel=cancel;
  const abort=()=>void cancel(signal.reason);
  signal?.addEventListener('abort',abort,{once:true});
  let timer;
  if(signal?.aborted)abort();
  if(timeoutMs===0)void cancel(new TaskTimeoutError(0));
  else if(timeoutMs!==undefined)timer=setTimeout(()=>void cancel(new TaskTimeoutError(timeoutMs)),timeoutMs);
  try{
    if(record.controller.signal.aborted)throw errorFor(record.controller.signal.reason);
    let value,bodyError,bodyFailed=false;
    try{value=await body(view(record));}catch(error){bodyFailed=true;bodyError=error;await cancel(error);}
    record.closed=true;
    await Promise.allSettled([...record.active].map(t=>t.result));
    if(record.failure)throw record.failure;
    if(bodyFailed&&!(record.controller.signal.aborted&&["TaskCancelledError","TaskTimeoutError"].includes(bodyError?.name)))throw bodyError;
    if(record.controller.signal.aborted)throw errorFor(record.controller.signal.reason);
    return value;
  }finally{
    record.closed=true;clearTimeout(timer);signal?.removeEventListener('abort',abort);
    await Promise.allSettled([...record.active].map(t=>t.cancel('scope exit')));
    record.active.clear();record.start=record.controller=record.cancel=record.failure=undefined;
  }
}
