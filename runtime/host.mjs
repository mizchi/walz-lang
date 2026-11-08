// Trusted event-loop bindings. No filesystem or network authority is supplied.
export function createEventLoop({ onEvent = () => {} } = {}) {
  function wait(milliseconds, value) {
    if (
      !Number.isInteger(milliseconds) ||
      milliseconds < 0 ||
      milliseconds > 2147483647
    ) {
      throw new WebAssembly.RuntimeError("invalid timer duration");
    }
    let finish;
    let active = true;
    let timer;
    const promise = new Promise((resolve) => {
      finish = resolve;
      timer = setTimeout(() => {
        active = false;
        onEvent({ type: "timer", milliseconds });
        resolve(value);
      }, milliseconds);
    });
    const cancel = () => {
      if (!active) return;
      active = false;
      clearTimeout(timer);
      onEvent({ type: "dispose", milliseconds });
      finish(value);
    };
    promise[Symbol.asyncDispose] = cancel;
    promise[Symbol.dispose] = cancel;
    onEvent({ type: "timer-start", milliseconds });
    return { promise, cancel };
  }
  return {
    after(milliseconds){return wait(milliseconds,undefined).promise;},
    sleep(milliseconds) {
      return wait(milliseconds, undefined).promise;
    },
    yield() {
      return wait(0, undefined).promise;
    },
    deferI64(milliseconds, value) {
      return wait(milliseconds, value).promise;
    },
    fallibleI64(milliseconds, code) {
      return wait(
        milliseconds,
        code === 0 ? { tag: "ok", val: 42n } : { tag: "err", val: code },
      ).promise;
    },
    ticks(count, milliseconds) {
      if (
        !Number.isInteger(count) ||
        count < 0 ||
        count > 65536 ||
        !Number.isInteger(milliseconds) ||
        milliseconds < 0 ||
        milliseconds > 2147483647
      )
        throw new WebAssembly.RuntimeError("invalid bounded stream");
      let index = 0;
      let closed = false;
      let pending;
      return {
        [Symbol.asyncIterator]() {
          return this;
        },
        async next() {
          if (closed || index === count) return { done: true };
          if (pending) throw new TypeError("concurrent stream read");
          const operation = wait(milliseconds, undefined);
          pending = operation;
          try {
            await operation.promise;
            if (closed) return { done: true };
            return { done: false, value: BigInt(index++) };
          } finally {
            if (pending === operation) pending = undefined;
          }
        },
        async return() {
          if (!closed) {
            closed = true;
            pending?.cancel();
            onEvent({ type: "stream-close" });
          }
          return { done: true };
        },
      };
    },
  };
}

export function createDelivery(factories) {
  const delivery = {};
  for (const { id, kind, unit } of factories) {
    delivery[`readyT${id}`] = (items) => {
      // The guest frees its canonical list buffer as soon as this call returns.
      const values = Array.from(items, (value) => (unit ? undefined : value));
      if (kind === "future") {
        if (values.length !== 1)
          throw new TypeError("future requires one value");
        return Promise.resolve(values[0]);
      }
      return prefix(values);
    };
    if (kind === "stream")
      delivery[`prependT${id}`] = (items, tail) =>
        prefix(
          Array.from(items, (value) => (unit ? undefined : value)),
          tail,
        );
  }
  return delivery;
}

function prefix(items, tail) {
  let index = 0;
  let closed = false;
  let iterator = tail?.[Symbol.asyncIterator]();
  return {
    [Symbol.asyncIterator]() {
      return this;
    },
    async next() {
      if (closed) return { done: true };
      if (index < items.length) {
        const value = items[index];
        items[index++] = undefined;
        if (index === items.length) {
          items.length = 0;
          index = 0;
        }
        return { done: false, value };
      }
      const result = iterator ? await iterator.next() : { done: true };
      if (result.done) {
        closed = true;
        iterator = null;
      }
      return result;
    },
    async readMany(max){
      if(!Number.isInteger(max)||max<1||max>65536)throw new RangeError('invalid batch count');
      if(closed)return {items:[],done:true};
      if(index<items.length){const end=Math.min(items.length,index+max),batch=items.slice(index,end);items.fill(undefined,index,end);index=end;return {items:batch,done:false};}
      const batch=iterator?.readMany?await iterator.readMany(max):iterator?await Promise.resolve(iterator.next()).then(r=>({items:r.done?[]:[r.value],done:r.done})):{items:[],done:true};
      if(batch.done){closed=true;iterator=null;}return batch;
    },
    async return() {
      if (!closed) {
        closed = true;
        items.length = 0;
        index = 0;
        const current = iterator;
        iterator = null;
        await current?.return?.();
      }
      return { done: true };
    },
  };
}

// Only explicitly supplied host objects grant I/O authority to a guest.
export function createHostImports(options, tasks, bindings = []) {
  const eventLoop = "wlt:host/event-loop@0.1.0";
  const hosts = Object.assign(Object.create(null), options.imports);
  hosts[eventLoop] = tasks.host(
    hosts[eventLoop] ?? options.eventLoop ?? createEventLoop(),
  );
  const original = {...hosts};
  for (const binding of bindings.filter(b=>b.resource)) {
    const {kind,operation}=binding.resource;
    const provider=original[binding.module];
    const fn=provider?.[binding.jsName];
    if(typeof fn!=="function")throw new TypeError(`missing resource provider ${binding.module}#${binding.name}`);
    const invoke=(...args)=>fn.apply(provider,args);
    hosts[binding.module]={...hosts[binding.module]};
    if(operation==="new") {
      const closer=bindings.find(b=>b.resource?.kind===kind && b.resource.operation==="close");
      const closeProvider=original[closer?.module],close=closeProvider?.[closer?.jsName];
      if(typeof close!=="function")throw new TypeError(`missing destructor for ${kind}`);
      hosts[binding.module][binding.jsName]=(...args)=>tasks.resources.add(kind,invoke(...args),v=>close.call(closeProvider,v));
    } else if(operation==="use" || operation==="reader") hosts[binding.module][binding.jsName]=(id,...args)=>tasks.resources.use(kind,id,invoke,args);
    else if(operation==='close') hosts[binding.module][binding.jsName]=id=>tasks.resources.close(kind,id);
    else throw new TypeError(`unsupported resource operation ${operation}`);
  }
  for (const namespace of new Set(bindings.map(b=>b.module))) {
    if (namespace !== eventLoop && hosts[namespace])
      hosts[namespace] = tasks.host(hosts[namespace],bindings.filter(b=>b.module===namespace && b.effects.suspends).map(b=>b.jsName));
  }
  for (const binding of bindings) {
    if (typeof hosts[binding.module]?.[binding.jsName] !== "function")
      throw new TypeError(
        `missing host capability ${binding.module}#${binding.name}`,
      );
  }
  for (const [namespace, host] of Object.entries(hosts))
    hosts[namespace.replace(/@[^@]+$/, "")] = host;
  return hosts;
}
