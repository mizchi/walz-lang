// Bounded scheduling, per-group cooperative cancellation, and joined leases.
const node=typeof process!=='undefined'&&!!process.versions?.node;
async function createWorker(){
  if(node){const {Worker}=await import('node:worker_threads');return new Worker(new URL('./agent.mjs',import.meta.url));}
  return new Worker(new URL('./agent.mjs',import.meta.url),{type:'module'});
}
function endpoint(w){
  let next=0,dead=false;const pending=new Map();
  const fail=error=>{dead=true;for(const p of pending.values())p.reject(error);pending.clear();};
  const receive=m=>{const p=pending.get(m.id);if(!p)return;pending.delete(m.id);if(m.type==='error')p.reject(new Error(m.message));else p.resolve(m);};
  if(node){w.on('message',receive);w.on('error',fail);w.on('exit',code=>fail(new Error(`worker exited (${code})`)));}
  else{w.addEventListener('message',e=>receive(e.data));w.addEventListener('error',e=>fail(new Error(e.message)));}
  return{
    request(type,data={},transfer=[]){
      if(dead)return Promise.reject(new Error('worker is closed'));
      const id=next++;
      return new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});try{w.postMessage({id,type,...data},transfer);}catch(e){pending.delete(id);reject(e);}});
    },
    notify(type,data){if(!dead)w.postMessage({type,...data});},
    async close(){if(!dead)try{await this.request('close');}finally{dead=true;await w.terminate();}else await w.terminate();},
  };
}
const abortError=()=>new DOMException('Worker host closed','AbortError');
// Keep completed handles outside a scheduler frame so retaining task history
// does not retain the host's shared-memory lease through a captured `this`.
function taskHandle(result,controller,external,forward){
  let finished=false;
  const finish=()=>{finished=true;external?.removeEventListener('abort',forward);};
  const done=result.then(finish,finish);
  const cancel=(reason=abortError())=>{if(!finished)controller.abort(reason);return done;};
  result[Symbol.asyncDispose]=()=>cancel();
  return Object.freeze({result,cancel,[Symbol.asyncDispose]:()=>cancel()});
}
export class WorkerHost{
  #slots=[];#queue=[];#groups=new Set();#next=0;#closed=false;#closing=null;#control=null;#lease;
  static async create({workers=1,driver,init,cancellation=null,workerFactory=createWorker}){
    if(!Number.isInteger(workers)||workers<1||workers>32)throw new RangeError('workers must be 1..32');
    const host=new WorkerHost();host.#lease=structuredClone(init);
    if(cancellation)cancellation={memory:cancellation.memory,byteOffset:cancellation.byteOffset};
    if(cancellation){
      const {memory,byteOffset}=cancellation;
      if(!(memory.buffer instanceof SharedArrayBuffer)||!Number.isSafeInteger(byteOffset)||byteOffset<0||byteOffset%4||byteOffset+4>memory.buffer.byteLength)throw new TypeError('cancellation requires an aligned shared i32 control cell');
      host.#control=new Int32Array(memory.buffer,byteOffset,1);
    }
    try{
      for(let id=0;id<workers;id++)host.#slots.push({id,endpoint:endpoint(await workerFactory()),job:null});
      await Promise.all(host.#slots.map(s=>s.endpoint.request('init',{workerId:s.id,driver:new URL(driver,import.meta.url).href,init:host.#lease,cancellation})));
      return host;
    }catch(error){await host.close();throw error;}
  }
  get size(){return this.#slots.length;}
  get busy(){return this.#groups.size>0;}
  start(payload,options={}){
    const controller=new AbortController(),external=options.signal;
    const forward=()=>controller.abort(external.reason);
    if(external?.aborted)forward();else external?.addEventListener('abort',forward,{once:true});
    return taskHandle(this.run(payload,{...options,signal:controller.signal}),controller,external,forward);
  }
  run(payload,options){return this.map([payload],options).then(results=>results[0]);}
  async map(payloads,{signal,transfer=()=>[]}={}){
    if(this.#closed)return Promise.reject(new Error('worker host is closed'));
    if(signal?.aborted)return Promise.reject(signal.reason);
    payloads=Array.from(payloads);
    if(this.#queue.length+payloads.length>4096)return Promise.reject(new RangeError('worker queue exceeds 4096 jobs; batch data within a job'));
    if(!payloads.length)return Promise.resolve([]);
    let resolve,reject;const done=new Promise((a,b)=>{resolve=a;reject=b;});
    const group={id:this.#next++,remaining:payloads.length,results:new Array(payloads.length),reason:null,cancelled:false,done,resolve,reject,signal,abort:null,transfer};
    group.abort=()=>this.#abort(group,signal?signal.reason:abortError());
    signal?.addEventListener('abort',group.abort,{once:true});
    this.#groups.add(group);payloads.forEach((payload,index)=>this.#queue.push({group,index,payload}));
    this.#pump();return done;
  }
  #finish(group){
    if(group.remaining)return;
    group.signal?.removeEventListener('abort',group.abort);this.#groups.delete(group);
    if(group.cancelled)group.reject(group.reason);else group.resolve(group.results);
  }
  #abort(group,reason){
    if(group.cancelled||!this.#groups.has(group))return;group.reason=reason;group.cancelled=true;
    this.#queue=this.#queue.filter(job=>{if(job.group!==group)return true;group.remaining--;return false;});
    for(const slot of this.#slots)if(slot.job?.group===group){
      if(this.#control)Atomics.or(this.#control,0,1<<slot.id);
      try{slot.endpoint.notify('cancel',{batch:group.id});}catch{}
    }
    this.#finish(group);
  }
  #pump(){
    if(this.#closed)return;
    for(const slot of this.#slots){
      if(slot.job||!this.#queue.length)continue;
      const job=this.#queue.shift(),g=job.group;slot.job=job;
      if(this.#control)Atomics.and(this.#control,0,~(1<<slot.id));
      let transfers;try{transfers=g.transfer(job.payload);}catch(error){transfers=[];this.#abort(g,error);}
      const pending=g.cancelled?Promise.reject(g.reason):slot.endpoint.request('run',{batch:g.id,payload:job.payload},transfers);
      pending.then(result=>{g.results[job.index]=result.value;},error=>this.#abort(g,error)).finally(()=>{
        slot.job=null;g.remaining--;this.#finish(g);this.#pump();
      });
    }
  }
  async join(){await Promise.allSettled([...this.#groups].map(g=>g.done));}
  close(){
    if(this.#closing)return this.#closing;
    this.#closed=true;const pending=this.join();
    for(const group of [...this.#groups])this.#abort(group,abortError());
    this.#closing=(async()=>{
      await pending;
      const results=await Promise.allSettled(this.#slots.map(s=>s.endpoint.close()));
      const failures=results.filter(r=>r.status==='rejected').map(r=>r.reason);
      if(failures.length)throw new AggregateError(failures,'worker cleanup failed; shared leases are retained');
      this.#slots=[];this.#control=null;this.#lease=null;
    })().catch(error=>{this.#closing=null;throw error;});
    return this.#closing;
  }
  [Symbol.asyncDispose](){return this.close();}
}
