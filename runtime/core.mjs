// Private, generated Core ABI. The engine owns GC objects and suspended stacks.
import { Tasks } from "./tasks.mjs";
import { createHostImports, createDelivery } from "./host.mjs";
export { TaskCancelledError } from "./tasks.mjs";

const cancelled = Symbol("cancelled read");
const noop = () => {};

// This wrapper can leave the instance: its methods capture only the producer.
// resolveAsValue preserves the distinction between returning a Future and
// awaiting it when the scheduler publishes a synchronous task's result.
class FutureValue {
  constructor(source) {
    this.source = source;
  }
  then(ok, fail) {
    return Promise.resolve(this.source).then(ok, fail);
  }
  resolveAsValue(resolve) {
    Object.defineProperty(this, "then", {
      value: undefined,
      configurable: true,
    });
    try {
      resolve(this);
    } finally {
      delete this.then;
    }
  }
  [Symbol.asyncDispose]() {
    const source = this.source;
    const dispose = source?.[Symbol.asyncDispose] ?? source?.[Symbol.dispose];
    return dispose?.call(source);
  }
}

function stream(value) {
  const get = value?.[Symbol.asyncIterator] ?? value?.[Symbol.iterator];
  if (typeof get !== "function") throw new TypeError("expected iterable");
  let iterator = get.call(value);
  return {
    [Symbol.asyncIterator]() {
      return this;
    },
    async next() {
      if (!iterator) return { done: true };
      const result = await iterator.next();
      if (result.done) iterator = null;
      return result;
    },
    async return() {
      const current = iterator;
      iterator = null;
      await current?.return?.();
      return { done: true };
    },
  };
}

class Boundary {
  constructor(memory, tasks, hosts) {
    this.allocator = memory.exports;
    this.memory = this.allocator.memory;
    this.tasks = tasks;
    this.hosts = hosts;
    this.imports = { ...memory.exports };
    this.definitions = new Map();
    this.handles = new Map();
    this.nextHandle = 1;
    this.promising = Object.create(null);
  }
  view() {
    return new DataView(this.memory.buffer);
  }
  alloc(size) {
    return this.allocator.alloc(size);
  }
  failHost(error, message) {
    if (error instanceof WebAssembly.Exception && error.is(this.tasks.tag))
      throw error;
    this.hostFailure ??=
      error instanceof Error && !message
        ? error
        : new WebAssembly.RuntimeError(message ?? "host operation failed", {
            cause: error,
          });
    throw this.hostFailure;
  }
  add(type, source) {
    const definition = this.definitions.get(type);
    if (definition.kind === "stream") source = stream(source);
    else {
      if (typeof source?.then !== "function")
        throw new TypeError("expected PromiseLike");
      Promise.resolve(source).catch(noop);
    }
    // Never recycle IDs: detached producer completions cannot address new ends.
    if (this.nextHandle > 0xffffffff)
      throw new RangeError("endpoint IDs exhausted");
    const handle = this.nextHandle++;
    this.handles.set(handle, { type, source });
    return handle;
  }
  entry(type, handle) {
    const entry = this.handles.get(handle >>> 0);
    if (!entry || entry.type !== type)
      throw new TypeError("invalid readable handle");
    return entry;
  }
  take(type, handle) {
    handle >>>= 0;
    const { source } = this.entry(type, handle);
    this.handles.delete(handle);
    return this.definitions.get(type).kind === "future"
      ? new FutureValue(source)
      : source;
  }
  drop(type, handle) {
    handle >>>= 0;
    const { source } = this.entry(type, handle);
    this.handles.delete(handle);
    const dispose =
      this.definitions.get(type).kind === "stream"
        ? source.return
        : (source?.[Symbol.asyncDispose] ?? source?.[Symbol.dispose]);
    if (dispose) this.tasks.disposal(() => dispose.call(source));
  }
  endpoint(type, kind, size, read, write) {
    this.definitions.set(type, { kind });
    const factory = createDelivery([{ id: type, kind, unit: false }]);
    const items = (p, n) =>
      Array.from({ length: n }, (_, i) => read(p + i * size));
    this.imports[`ready${type}`] = (p, n) =>
      this.add(type, factory[`readyT${type}`](items(p, n)));
    if (kind === "stream")
      this.imports[`prepend${type}`] = (p, n, tail) =>
        this.add(
          type,
          factory[`prependT${type}`](items(p, n), this.take(type, tail)),
        );
    this.imports[`drop${type}`] = (handle) => this.drop(type, handle);
    this.imports[`read${type}`] = new WebAssembly.Suspending((handle, p) => {
      if (this.hostFailure) throw this.hostFailure;
      const { source } = this.entry(type, handle);
      const stop = Promise.withResolvers();
      let active = true;
      const cancel = () => {
        active = false;
        stop.resolve(cancelled);
      };
      // Put the write after the race. A late producer keeps no callback capable
      // of writing into an arena already reclaimed by cancellation.
      const pending =
        kind === "future" ? Promise.resolve(source) : source.next();
      return this.tasks.read(cancel, () =>
        Promise.race([pending, stop.promise])
          .then((value) => {
            if (!active || value === cancelled) return 1;
            if (kind === "future") {
              write(p, value);
              return 0;
            }
            if (value.done) return 1;
            write(p, value.value);
            return 16; // one element, readable remains open
          })
          .catch((error) => this.failHost(error, "promise failed")),
      );
    });
  }
  hostImport(
    slot,
    name,
    suspends,
    convert,
    namespace = "wlt:host/event-loop@0.1.0",
    decode = (args) => args,
  ) {
    const host = this.hosts[namespace];
    if (typeof host?.[name] !== "function")
      throw new TypeError(`missing host capability ${namespace}#${name}`);
    const invoke = (...args) => {
      if (this.hostFailure) throw this.hostFailure;
      try {
        const value = host[name](...decode(args));
        return suspends
          ? Promise.resolve(value)
              .then(v => convert(v,args))
              .catch((error) => this.failHost(error))
          : convert(value,args);
      } catch (error) {
        return this.failHost(error);
      }
    };
    this.imports[slot] = suspends ? new WebAssembly.Suspending(invoke) : invoke;
  }
  call(name, arity, size, write, read, post, async, argc) {
    if (arity !== argc) throw new TypeError(`expected ${arity} arguments`);
    const firstHandle = this.nextHandle;
    let p = 0;
    try {
      if (arity) p = this.alloc(size);
      write(p);
    } catch (error) {
      // Validation failed before entering the guest. Roll back only these inputs.
      for (const [handle, entry] of this.handles)
        if (handle >= firstHandle) this.drop(entry.type, handle);
      this.allocator["free-all"]();
      throw error;
    }
    const finish = (pointer) => {
      if (this.hostFailure) throw this.hostFailure;
      if (!post) return undefined;
      try {
        return read(pointer);
      } finally {
        this.exports[post](pointer);
      }
    };
    const failed = (error) => {
      if (!(
        error instanceof WebAssembly.Exception && error.is(this.tasks.tag)
      )) {
        this.tasks.failure = new Error(
          "cannot enter component instance after failure",
          { cause: error },
        );
        // Retire registered guest ends before draining partially converted ends.
        // finish() may invoke this again; the guest cleanup is idempotent.
        try { this.tasks.cleanupGuest(); }
        catch (cleanupError) { error = new AggregateError([error,cleanupError], "guest cleanup failed"); }
        for (const [handle, entry] of this.handles)
          this.drop(entry.type, handle);
        this.allocator["free-all"]();
      }
      throw error;
    };
    const args = arity ? [p] : [];
    try {
      if (async) {
        const fn = (this.promising[name] ??= WebAssembly.promising(
          this.exports[name],
        ));
        return fn(...args)
          .then(finish)
          .catch(failed);
      }
      return finish(this.exports[name](...args));
    } catch (error) {
      return failed(error);
    }
  }
}

export async function instantiate(
  base,
  abi,
  bind,
  options = {},
  bindings = [],
) {
  if (!["wlt-core-js-v1", "wlt-core-js-v2"].includes(abi))
    throw new Error("unsupported Core JS ABI");
  const load =
    options.getCoreModule ??
    (async (name) => {
      const url = new URL(name, base);
      if (
        url.protocol === "file:" &&
        typeof process !== "undefined" &&
        process.versions?.node
      )
        return WebAssembly.compile(
          await (await import("node:fs/promises")).readFile(url),
        );
      return WebAssembly.compileStreaming(fetch(url));
    });
  const tasks = new Tasks(options);
  const hosts = createHostImports(options, tasks, bindings);
  const create = async (name, imports) =>
    tasks.capture(
      await (options.instantiateCore ?? WebAssembly.instantiate)(
        await load(name),
        imports,
      ),
    );
  const memory = await create("memory.wasm", {});
  const boundary = new Boundary(memory, tasks, hosts);
  const { api, asyncExports } = bind(boundary);
  tasks.asyncExports = new Set(asyncExports);
  const guest = await create("core.wasm", { __component: boundary.imports });
  boundary.exports = guest.exports;
  return tasks.bind({ api });
}
