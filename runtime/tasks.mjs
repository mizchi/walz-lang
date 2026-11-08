import {Resources} from "./resources.mjs";
// One active guest stack per instance. Requests, guest unwinding and host
// disposal are separate stages; only the last stage publishes an acknowledgment.
function materializeError(error, seen) {
  if (!(error instanceof Error)) return;
  seen ??= new Set();
  if (seen.has(error)) return;
  seen.add(error);
  // V8's lazy CallSites can retain the guest and its closures after a failed
  // task. Read ordinary stacks before publishing errors, including cleanup
  // failures and causes, without replacing the original error or its identity.
  try {
    void error.stack;
  } catch {
    // Custom diagnostic getters must not replace the original failure.
  }
  materializeError(
    Object.getOwnPropertyDescriptor(error, "cause")?.value,
    seen,
  );
  const errors = Object.getOwnPropertyDescriptor(error, "errors")?.value;
  if (Array.isArray(errors))
    for (const nested of errors) materializeError(nested, seen);
}

export class TaskCancelledError extends Error {
  constructor(reason,message="Task cancelled",name="TaskCancelledError") {
    super(message, { cause: reason });
    this.name = name;
    materializeError(this);
  }
}
export class TaskTimeoutError extends TaskCancelledError {
  constructor(milliseconds){super(undefined,`Task timed out after ${milliseconds} ms`,"TaskTimeoutError");this.timeoutMs=milliseconds;}
}
const cancellationError=reason=>reason instanceof TaskTimeoutError?reason:new TaskCancelledError(reason);
const terminal = new Set(["returned", "cancelled", "failed"]);
function deferred() {
  return Promise.withResolvers();
}

// Construct handles outside the scheduler's method scope: keeping completed
// task history must not keep the instance alive through a captured `this`.
function taskHandle(record) {
  return Object.freeze({
    get state() {
      return record.state;
    },
    result: record.result.promise,
    cancel(reason) {
      return record.cancel ? record.cancel(reason) : record.done.promise;
    },
  });
}

export class Tasks {
  constructor({ onTaskEvent = () => {}, asyncExports = [] } = {}) {
    this.notify = onTaskEvent;
    this.asyncExports = new Set(asyncExports);
    this.queue = [];
    this.active = null;
    this.failure = null;
    this.nextId = 1;
    this.resources = new Resources(()=>this.active,fn=>this.disposal(fn),()=>this.scope);
    this.children = null;
    this.scope=0;
  }
  capture(instance) {
    if(instance.exports["scope-owner"])this.scopeOwner=instance.exports["scope-owner"];
    if(instance.exports["free-scope"])this.freeScope=instance.exports["free-scope"];
    if(instance.exports.__child_discard)this.discardChild=instance.exports.__child_discard;
    if(instance.exports.__scope_cleanup)this.cleanupScope=instance.exports.__scope_cleanup;
    if(instance.exports.__child_run)this.childRun=WebAssembly.promising(instance.exports.__child_run);
    if(instance.exports.__children_clear)this.clearChildren=instance.exports.__children_clear;
    if (instance.exports["free-all"])
      this.freeBoundary = instance.exports["free-all"];
    if (instance.exports.__task_cleanup)
      this.guestCleanup = instance.exports.__task_cleanup;
    if (instance.exports["cancel-requested"])
      this.flag = instance.exports["cancel-requested"];
    if (instance.exports.__task_cancel)
      this.tag = instance.exports.__task_cancel;
    return instance;
  }
  enter(scope) {this.scope=scope;if(this.scopeOwner)this.scopeOwner.value=scope;}
  cancelled(scope=this.scope){return this.active?.state==="cancelling" || !!this.active?.children?.scopes.get(scope)?.cancelled;}
  cleanupGuest() {
    this.cleaning = true;
    try { this.guestCleanup?.(); }
    finally { this.cleaning = false; }
  }
  bind(component) {
    if(component.wltChildRun)this.childRun=component.wltChildRun;
    const {wltChildRun:privateRunner,...visible}=component;
    if (!this.flag || !this.tag)
      throw new Error("component lacks the cancellation ABI");
    this.raw = component.api;
    const api = Object.fromEntries(
      Object.entries(this.raw).map(([name, fn]) => [
        name,
        typeof fn === "function" ? (...args) => this.invoke(name, args) : fn,
      ]),
    );
    // Both aliases must use the scheduler; raw exports are never exposed.
    return {
      ...visible,
      api,
      "wlt:generated/api@0.1.0": api,
      start: this.start.bind(this),
    };
  }
  create(name, args, options = {}) {
    if (!Object.hasOwn(this.raw, name) || typeof this.raw[name] !== "function")
      throw new TypeError(`unknown export ${name}`);
    if (!Array.isArray(args))
      throw new TypeError("task arguments must be an array");
    if(options.timeoutMs!==undefined&&(!Number.isInteger(options.timeoutMs)||options.timeoutMs<0||options.timeoutMs>2147483647))throw new RangeError('invalid timeoutMs');
    if(options.signal&&typeof options.signal.addEventListener!=='function')throw new TypeError('expected AbortSignal');
    const result = deferred(),
      done = deferred();
    result.promise.catch(() => {});
    const record = {
      id: this.nextId++,
      name,
      args: [...args],
      state: "queued",
      started: false,
      settling: false,
      reason: undefined,
      result,
      done,
      disposals: [],
      reads: new Set(),
      blocking: new Set(),
    };
    const cancel = (reason) => {
      if (terminal.has(record.state) || record.settling) return done.promise;
      if (record.state === "cancelling") return done.promise;
      record.reason = reason;
      if (!record.started) {
        record.state = "cancelled";
        record.cancel = undefined;
        record.detach?.();
        record.detach = undefined;
        record.args = [];
        result.reject(cancellationError(reason));
        done.resolve({ status: "cancelled", started: false });
        return done.promise;
      }
      record.state = "cancelling";
      this.flag.value = 1;
      for (const cancelRead of record.reads) cancelRead();
      for (const operation of record.blocking) operation();
      return done.promise;
    };
    record.cancel = cancel;
    record.public = taskHandle(record);
    let timer;
    const signal=options.signal;
    const abort=()=>cancel(signal.reason);
    record.detach=()=>{clearTimeout(timer);signal?.removeEventListener("abort",abort);};
    signal?.addEventListener("abort",abort,{once:true});
    if(signal?.aborted)abort();
    if(!terminal.has(record.state)&&options.timeoutMs!==undefined){
      if(options.timeoutMs===0)cancel(new TaskTimeoutError(0));
      else timer=setTimeout(()=>cancel(new TaskTimeoutError(options.timeoutMs)),options.timeoutMs);
    }
    return record;
  }
  start(name, args = [], options = {}) {
    const record = this.create(name, args, options);
    this.queue.push(record);
    queueMicrotask(() => this.pump());
    return record.public;
  }
  pump() {
    if (this.active) return;
    while (this.queue.length) {
      const record = this.queue.shift();
      if (record.state === "queued") {
        this.execute(record);
        return;
      }
    }
  }
  invoke(name, args) {
    if (this.failure) {
      if (this.asyncExports.has(name)) return Promise.reject(this.failure);
      throw this.failure;
    }
    if (this.active || this.queue.length) {
      if (!this.asyncExports.has(name))
        throw new Error(
          "component is busy; use start() to queue a synchronous export",
        );
      return this.start(name, args).result;
    }
    const record = this.create(name, args);
    return this.execute(record, true);
  }
  execute(record, inline = false) {
    this.active = record;
    record.started = true;
    record.state = "running";
    this.flag.value = 0;
    this.enter(0);
    let value;
    try {
      if (this.failure) throw this.failure;
      value = this.raw[record.name](...record.args);
    } catch (error) {
      record.args = [];
      this.finish(record, undefined, error);
      if (inline && !this.asyncExports.has(record.name)) throw error;
      return record.result.promise;
    }
    record.args = [];
    if (this.asyncExports.has(record.name)) {
      Promise.resolve(value).then(
        (value) => this.finish(record, value),
        (error) => this.finish(record, undefined, error),
      );
      return record.result.promise;
    }
    try { this.resources.closeScope(record); } catch(error) {
      this.failure=error; this.finish(record,undefined,error);
      if(inline)throw error;
      return record.result.promise;
    }
    if (record.disposals.length) {
      this.finish(record, value);
      return inline ? value : record.result.promise;
    }
    this.publish(record, value);
    return value;
  }
  async finish(record, value, error) {
    record.settling = true;
    try {
      const failures = [];
      if(record.children){await this.children.finish(record,error);error=record.childFailure??error;}
      if (error) {
        try { this.cleanupGuest(); }
        catch (cleanupError) { failures.push(cleanupError); }
      }
      // Jco schedules disposal when readable ends are dropped. The bridge
      // registers each disposal before its microtask starts, so no timer race
      // or fixed grace period is needed here.
      while (record.disposals.length) {
        const results = await Promise.allSettled(record.disposals.splice(0));
        failures.push(
          ...results
            .filter((x) => x.status === "rejected")
            .map((x) => x.reason),
        );
      }
      try {this.resources.closeScope(record);} catch(error) {failures.push(error);}
      while(record.disposals.length) {
        const results=await Promise.allSettled(record.disposals.splice(0));
        failures.push(...results.filter(r=>r.status==="rejected").map(r=>r.reason));
      }
      if(this.resources.failure)failures.push(this.resources.failure);
      if (failures.length)
        throw new AggregateError(failures, "task cleanup failed");
    } catch (cleanupError) {
      this.failure = cleanupError;
      error = cleanupError;
    }
    if (error) {
      try { this.freeBoundary?.(); }
      catch (cleanupError) {
        error = new AggregateError([error, cleanupError], "boundary cleanup failed");
        this.failure = error;
      }
    }
    const cancelled =
      record.state === "cancelling" &&
      error instanceof WebAssembly.Exception &&
      error.is(this.tag);
    this.publish(record, value, error, cancelled);
  }
  publish(record, value, error, cancelled = false) {
    materializeError(error);
    record.state = cancelled ? "cancelled" : error ? "failed" : "returned";
    record.cancel = undefined;
    record.detach?.();
    record.detach = undefined;
    if (cancelled) record.result.reject(cancellationError(record.reason));
    else if (error) record.result.reject(error);
    else if (typeof value?.resolveAsValue === "function")
      value.resolveAsValue(record.result.resolve);
    else record.result.resolve(value);
    record.done.resolve({ status: record.state, started: record.started });
    this.active = null;
    this.flag.value = 0;
    queueMicrotask(() => this.pump());
  }
  disposal(thunk) {
    const operation = Promise.resolve().then(thunk);
    if (this.active) {
      this.active.disposals.push(operation);
      const scope=this.active.children?.scopes.get(this.scope);
      scope?.disposals.push(operation);
    }
    else
      operation.catch((error) => console.error("host disposal failed", error));
    // A cancellation request may await only done, rather than result.
    operation.catch(() => {});
    return operation;
  }
  read(cancel, run) {
    const scope=this.scope;
    const value = run();
    if (!value || typeof value.then !== "function") return value;
    const record = this.active;
    if (!record) return value;
    cancel.scope=scope;
    record.reads.add(cancel);
    this.notify({ type: "read-pending", id: record.id });
    if (this.cancelled(scope)) cancel();
    return Promise.resolve(value).finally(() => {this.enter(scope);record.reads.delete(cancel);});
  }
  host(host, blocking = ["sleep", "yield"]) {
    return Object.fromEntries(
      Object.entries(host).map(([name, fn]) => [
        name,
        typeof fn !== "function"
          ? fn
          : (...args) => {
              const scope=this.scope;
              const value = fn.apply(host, args),
                record = this.active;
              if (!record || !blocking.includes(name)) return value;
              let disposed = false, cleanup;
              const cancel = () => {
                if (disposed) return;
                disposed = true;
                const dispose =
                  value?.[Symbol.asyncDispose] ?? value?.[Symbol.dispose];
                if (dispose) cleanup = this.disposal(() => dispose.call(value));
              };
              cancel.scope=scope;
              record.blocking.add(cancel);
              this.notify({type:'host-pending',id:record.id,name});
              if (this.cancelled(scope)) cancel();
              const cancelled = async () => {
                // The host operation may reject with AbortError. Complete its
                // disposal before unwinding guest using scopes and closing resources.
                await cleanup;
                throw new WebAssembly.Exception(this.tag, []);
              };
              return Promise.resolve(value).then(
                value => {this.enter(scope);return this.cancelled(scope) ? cancelled() : value;},
                error => {
                  this.enter(scope);if (this.cancelled(scope)) return cancelled();
                  throw error;
                },
              ).finally(() => record.blocking.delete(cancel));
            },
      ]),
    );
  }
}
