// Structured children of one API call, with per-scope request/acknowledgment.
export class Children {
  static core(tasks,boundary){
    tasks.children=new Children(tasks);
    for(const [name,fn] of Object.entries(tasks.children.host())){
      const key=name.replace(/[A-Z]/g,c=>'-'+c.toLowerCase());
      boundary.imports[key]=['scope-close','child-join','child-cancel','child-timeout'].includes(key)?new WebAssembly.Suspending(fn):fn;
    }
  }
  constructor(tasks){this.tasks=tasks;this.next=1;}
  state(){const root=this.tasks.active;if(!root)throw new TypeError('child tasks require an active root task');return root.children??={scopes:new Map(),jobs:new Map()};}
  id(){if(this.next>0x7fffffff)throw new RangeError('task IDs exhausted');return this.next++;}
  scopeNew(parent){
    const state=this.state();if(parent&&!state.scopes.has(parent))throw new TypeError('invalid task scope');
    const id=this.id();state.scopes.set(id,{id,parent,jobs:new Set(),pending:new Set(),closing:false,cancelled:!!state.scopes.get(parent)?.cancelled,disposals:[]});return id;
  }
  scopeEnter(id){this.tasks.enter(id);return this.tasks.cancelled(id)?1:0;}
  childNew(scope){
    const state=this.state(),group=state.scopes.get(scope);if(!group||group.closing)throw new TypeError('closed task scope');
    const id=this.id(),done=Promise.withResolvers(),job={id,group,scope:this.scopeNew(scope),done,started:false,finished:false};
    state.scopes.get(job.scope).job=true;state.jobs.set(id,job);this.track(group,job);return id;
  }
  track(group,job){group.jobs.add(job);const p=job.done.promise;group.pending.add(p);p.then(()=>group.pending.delete(p));}
  childAdopt(id,scope){
    const state=this.state(),job=state.jobs.get(id),group=state.scopes.get(scope);
    if(!job||!group||group.closing)throw new TypeError('invalid child move');
    job.group.jobs.delete(job);job.group=group;this.track(group,job);if(state.scopes.has(job.scope))state.scopes.get(job.scope).parent=scope;
    if(group.cancelled)this.cancelScope(job.scope);
  }
  childStart(id){
    const state=this.state(),root=this.tasks.active,job=state.jobs.get(id);if(!job||job.started)throw new TypeError('invalid task start');job.started=true;
    queueMicrotask(async()=>{
      this.tasks.notify({type:'child-started',id});
      try{await this.tasks.childRun(id,job.scope);}
      catch(error){job.error=error;if(!(job.cancelled&&error instanceof WebAssembly.Exception&&error.is(this.tasks.tag))&&root.state!=='cancelling'&&!root.childrenAborting){root.childFailure??=error;root.cancel?.(error);}}
      finally{
        try{await this.scopeClose(job.scope);}catch(error){job.error=error;root.childFailure??=error;root.cancel?.(error);}
        job.finished=true;this.tasks.notify({type:'child-finished',id});job.done.resolve();
      }
    });
  }
  retire(job){const state=this.state();state.jobs.delete(job.id);job.group.jobs.delete(job);}
  async childJoin(id){
    const job=this.state().jobs.get(id);if(!job||!job.started)throw new TypeError('invalid task join');
    await job.done.promise;this.retire(job);if(job.error&&!job.cancelled&&this.tasks.active.state!=='cancelling')throw job.error;
  }
  cancelScope(id){
    const state=this.state(),ids=new Set([id]);let changed=true;
    while(changed){changed=false;for(const g of state.scopes.values())if(ids.has(g.parent)&&!ids.has(g.id)){ids.add(g.id);changed=true;}}
    for(const n of ids){const g=state.scopes.get(n);if(g)g.cancelled=true;}
    for(const job of state.jobs.values())if(ids.has(job.scope))job.cancelled=true;
    for(const set of [this.tasks.active.reads,this.tasks.active.blocking])for(const cancel of set)if(ids.has(cancel.scope))cancel();
  }
  async childCancel(id){const job=this.state().jobs.get(id);if(!job)throw new TypeError('invalid task cancellation');if(!job.finished)this.cancelScope(job.scope);await this.childJoin(id);}
  async childTimeout(id,ms){
    if(!Number.isInteger(ms)||ms<0)throw new RangeError('timeout must be a non-negative i32');
    const job=this.state().jobs.get(id);if(!job)throw new TypeError('invalid task timeout');let timer;
    try{
      if(!job.finished){const timeout=new Promise(resolve=>{timer=setTimeout(()=>resolve(true),ms);});
        if(await Promise.race([job.done.promise.then(()=>false),timeout])){if(!job.finished){this.cancelScope(job.scope);await this.childJoin(id);return 1;}}}
      await this.childJoin(id);return 0;
    }finally{clearTimeout(timer);}
  }
  async scopeClose(id){
    const state=this.state(),group=state.scopes.get(id);if(!group)return;group.closing=true;
    await Promise.all([...group.pending]);
    this.tasks.enter(id);
    if(group.cancelled||group.job){
      this.tasks.cleanupScope?.(id);this.tasks.enter(id);this.tasks.resources.closeScope(this.tasks.active,id);
      for(const job of group.jobs){this.retire(job);this.tasks.discardChild?.(job.id);}
    }
    while(group.disposals.length){const pending=group.disposals.splice(0);const results=await Promise.allSettled(pending);const retired=new Set(pending);this.tasks.active.disposals=this.tasks.active.disposals.filter(p=>!retired.has(p));const errors=results.filter(r=>r.status==='rejected').map(r=>r.reason);if(errors.length)throw new AggregateError(errors,'child cleanup failed');}
    if(group.cancelled)this.tasks.freeScope?.(id);state.scopes.delete(id);
  }
  abort(reason,request=true){const root=this.tasks.active;if(!root?.children)return;root.childrenAborting=true;if(request)root.cancel?.(reason);for(const job of root.children.jobs.values())if(!job.started)job.done.resolve();}
  async drain(root){if(!root.children)return;await Promise.all([...root.children.jobs.values()].map(job=>job.done.promise));root.children.jobs.clear();root.children.scopes.clear();root.children=undefined;}
  async finish(root,error){
    if(error){this.abort(error,false);this.tasks.flag.value=1;for(const cancel of root.reads)cancel();for(const cancel of root.blocking)cancel();}
    await this.drain(root);this.tasks.clearChildContexts?.();this.tasks.clearChildren?.();this.tasks.enter(0);
  }
  host(){return {scopeNew:this.scopeNew.bind(this),scopeClose:this.scopeClose.bind(this),scopeAbort:id=>{if(this.state().scopes.get(id)?.cancelled)this.cancelScope(id);else this.abort('task scope unwound');},scopeEnter:this.scopeEnter.bind(this),childNew:this.childNew.bind(this),childStart:this.childStart.bind(this),childJoin:this.childJoin.bind(this),childCancel:this.childCancel.bind(this),childTimeout:this.childTimeout.bind(this),childAdopt:this.childAdopt.bind(this),resourceAdopt:(id,scope)=>this.tasks.resources.adopt(id,scope)};}
}
