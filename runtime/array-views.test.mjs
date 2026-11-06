import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { instrument, tick, collect } from './testing/memory.mjs';

for (const backend of ['core', 'component']) {
  test(`${backend}: array views materialize only visible elements and survive suspension`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'wlt-array-views-'));
    try {
      const { build } = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/array-views.wlt', directory);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, 'index.js')));
      const make = async () => {
        let arena;
        const runtime = await instantiate({ instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        } });
        return { ...runtime, clean() {
          assert.equal(arena.live(), 0);
          assert.ok(Object.values(runtime.__memory()).every(n => n === 0));
        } };
      };
      const { api, clean } = await make();
      let seed = 23;
      const next = n => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed % n; };
      for (let round = 0; round < 60; round++) {
        const input = Array.from({ length: next(128) }, () => BigInt(next(10000) - 5000));
        const a = next(input.length + 1), n = next(input.length - a + 1);
        const b = next(n + 1), m = next(n - b + 1);
        assert.deepEqual(Array.from(api.slice(input, a, n, b, m)), input.slice(a + b, a + b + m));
        if (n) assert.equal(api.get(input, a, n, n - 1), input[a + n - 1]);
        clean();
      }
      let resume;
      const pause = new Promise(resolve => { resume = resolve; });
      const pending = api.after(Array(65536).fill(42n), 65534, 2, pause);
      await tick();
      if (global.gc) await collect();
      resume();
      assert.deepEqual(Array.from(await pending), [42n, 42n]);
      clean();
      for (const [method, args] of [
        ['slice', [[1n, 2n, 3n], -1, 0, 0, 0]],
        ['slice', [[1n, 2n, 3n], 0, -1, 0, 0]],
        ['slice', [[1n, 2n, 3n], 1, 1, 0, 2]],
        ['slice', [[1n, 2n, 3n], 2147483647, 2147483647, 0, 0]],
        ['get', [[1n, 2n, 3n], 0, 1, 1]],
        ['get', [[1n, 2n, 3n], 0, 1, -1]],
      ]) {
        const bad = await make();
        assert.throws(() => bad.api[method](...args));
        bad.clean();
      }
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
}
