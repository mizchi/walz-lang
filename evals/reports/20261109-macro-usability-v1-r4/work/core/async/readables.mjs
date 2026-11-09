// Optional readable transport; scalar-only bundles omit this module.
import {createDelivery} from './host.mjs';
const cancelled=Symbol('cancelled read');
export function install(boundary){boundary.wrapStream=stream;boundary.endpoint=endpoint.bind(boundary);}
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
    async readMany(max) {
      if(!iterator)return {items:[],done:true};
      if(iterator.readMany)return iterator.readMany(max);
      const result=await iterator.next();
      if(result.done){iterator=null;return {items:[],done:true};}
      return {items:[result.value],done:false};
    },
    async return() {
      const current = iterator;
      iterator = null;
      await current?.return?.();
      return { done: true };
    },
  };
}

export function endpoint(type, kind, size, read, write) {
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
    this.imports[`read${type}`] = new WebAssembly.Suspending((handle, p, max=1) => {
      if (this.hostFailure) throw this.hostFailure;
      const { source } = this.entry(type, handle);
      const scope = this.tasks.scope;
      const stop = Promise.withResolvers();
      let active = true;
      const cancel = () => {
        active = false;
        stop.resolve(cancelled);
      };
      // Put the write after the race. A late producer keeps no callback capable
      // of writing into an arena already reclaimed by cancellation.
      const pending =
        kind === "future" ? Promise.resolve(source) : source.readMany(max);
      return this.tasks.read(cancel, () =>
        Promise.race([pending, stop.promise])
          .then((value) => {
            if (!active || value === cancelled) return 1;
            this.tasks.enter(scope);
            if (kind === "future") {
              write(p, value);
              return 0;
            }
            if(!Number.isInteger(max)||max<1||max>65536||!value||!value.items||!Number.isInteger(value.items.length)||value.items.length>max||(!value.items.length&&!value.done))throw new TypeError('invalid stream batch');
            for(let i=0;i<value.items.length;i++)write(p+i*size,value.items[i]);
            return (value.items.length<<4)|(value.done?1:0);
          })
          .catch((error) => this.failHost(error, "promise failed")),
      );
    });
  }
