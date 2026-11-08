// Task-scoped external resources. GC values never own OS handles implicitly.
export class Resources {
  constructor(active, disposal, owner=()=>0) {
    this.active=active; this.disposal=disposal;this.owner=owner;
    this.entries=new Map(); this.next=1;
  }
  add(kind, value, close) {
    const scope=this.active();
    if(!scope)throw new TypeError('resource factory requires an active task');
    if(this.next>0x7fffffff) { close(value); throw new RangeError('resource IDs exhausted'); }
    if(value && typeof value.then==="function") {
      this.disposal(async()=>close(await value));
      throw new TypeError("resource factory must be synchronous");
    }
    const id=this.next++;
    this.entries.set(id,{kind,value,close,scope,owner:this.owner()});
    scope.resources??=new Set(); scope.resources.add(id);
    return id;
  }
  entry(kind,id) {
    const e=this.entries.get(id);
    if(!e || e.kind!==kind || e.scope!==this.active())throw new TypeError('invalid, closed, or foreign resource');
    return e;
  }
  use(kind,id,fn,args) { return fn(this.entry(kind,id).value,...args); }
  close(kind,id) {
    const e=this.entry(kind,id);
    this.entries.delete(id); e.scope.resources.delete(id);
    let value;
    try{value=e.close(e.value);}catch(error){this.failure??=error;throw error;}
    if(value && typeof value.then==='function') {
      this.disposal(()=>value);
      this.failure??=new TypeError('resource destructor must be synchronous');
      throw this.failure;
    }
  }
  adopt(id,owner){const e=this.entries.get(id);if(!e || e.scope!==this.active())throw new TypeError("invalid resource move");e.owner=owner;}
  closeScope(scope,owner) {
    const errors=[];
    for(const id of [...(scope.resources??[])].reverse()) {
      const e=this.entries.get(id);
      if(owner!==undefined&&e.owner!==owner)continue;
      try {this.close(e.kind,id);}catch(error){errors.push(error);}
    }
    if(errors.length)throw new AggregateError(errors,'resource cleanup failed');
  }
}
