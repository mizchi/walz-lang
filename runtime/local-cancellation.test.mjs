import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument} from './testing/memory.mjs';
import {createEventLoop} from './host.mjs';

for (const backend of ['core', 'component']) {
  test(`${backend}: local cancellation preserves siblings and releases captures`, {timeout: 60000}, async () => {
    const dir = await mkdtemp(join(tmpdir(), 'wlt-local-cancel-'));
    try {
      const {build} = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/local-cancellation.wlt', dir);
      await instrument(dir, backend);
      const {instantiate} = await import(pathToFileURL(join(dir, 'index.js')));
      const events = [], lowered = [], owners = new Map(), acknowledged = [];
      let allocator, guest;
      const clock = createEventLoop({onEvent: event => events.push(event)});
      const disposed = () => events.filter(e => e.type === 'dispose' && e.milliseconds === 60000).length;
      const c = await instantiate({
        eventLoop: {
          ...clock,
          sleep(n) {
            if (n === 3) acknowledged.push(disposed());
            if (n === 1 || n === 2) owners.set(n, allocator['scope-owner'].value);
            return n < 60000 ? new Promise(resolve => setTimeout(resolve, n)) : clock.sleep(n);
          },
        },
        async instantiateCore(module, imports) {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.__children_live) guest = instance.exports;
          if (!instance.exports.live) return instance;
          allocator = instance.exports;
          // Observe only boundary allocations; internal Wasm calls remain native.
          const exports = {...allocator};
          for (const key of ['alloc', 'realloc']) {
            if (!exports[key]) continue;
            exports[key] = (...args) => {
              const size = args.at(-1);
              if (size > 0) lowered.push({scope: allocator['scope-owner'].value, size});
              return allocator[key](...args);
            };
          }
          return {exports};
        },
      });
      const empty = () => {
        assert.equal(guest.__children_live(), 0);
        assert.equal(allocator.live(), 0);
        assert.ok(Object.values(c.__memory()).every(n => n === 0), JSON.stringify(c.__memory()));
      };
      for (let n = 0; n < 5; n++) {
        assert.equal(await c.api.run(), 42);
        assert.equal(await c.api.cancel(), 42);
        assert.equal(await c.api.ready(), 21);
        assert.equal(await c.api.running(), undefined);
        assert.equal(await c.api.nested(), undefined);
        empty();
      }
      const before = disposed();
      await c.api.escaped();
      assert.deepEqual(acknowledged, [before + 2], 'parent must resume after both escaped future and sleep are disposed');
      empty();

      const text = '漢😀'.repeat(16384);
      lowered.length = 0;
      owners.clear();
      const delayed = (ms, value) => new Promise(resolve => setTimeout(() => resolve(value), ms));
      assert.equal(await c.api.text(delayed(8, 'first'), delayed(15, text)), text);
      assert.equal(lowered.find(x => x.size === 5)?.scope, owners.get(1), JSON.stringify(lowered));
      assert.equal(lowered.find(x => x.size === new TextEncoder().encode(text).length)?.scope, owners.get(2), JSON.stringify(lowered));
      empty();
      assert.ok(disposed() > 0);
    } finally {
      await rm(dir, {recursive: true, force: true});
    }
  });
}
