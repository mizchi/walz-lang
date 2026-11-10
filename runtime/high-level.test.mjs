import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument} from './testing/memory.mjs';

for (const backend of ['core', 'component']) {
  test(`${backend}: high-level forms preserve order, snapshots, exceptions and ABI cleanup`, {timeout:60000}, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'walz-high-level-'));
    try {
      const {build} = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/high-level.walz', directory);
      await instrument(directory, backend);
      const {instantiate} = await import(pathToFileURL(join(directory, 'index.js')));
      let memory;
      const {api, __memory} = await instantiate({instantiateCore: async (m,i) => {
        const instance = await WebAssembly.instantiate(m,i);
        if (instance.exports.live) memory = instance.exports;
        return instance;
      }});
      const clean = () => {
        assert.equal(memory.live(), 0);
        assert.ok(Object.values(__memory()).every(n => n === 0));
      };
      const inputs = [[], [[10n,1],[30n,1]], [[-9223372036854775808n,-1],[-1n,3]],
        Array.from({length:1000}, (_,i) => [BigInt(i - 500), i % 5])];
      for (let repeat = 0; repeat < 10; repeat++) {
        for (const input of inputs) {
          const total = BigInt.asIntN(64, input.reduce((n,[v]) => n + v, 0n));
          const count = BigInt(input.reduce((n,[,v]) => (n + v) | 0, 0));
          assert.deepEqual([...api.sum(input)], [total, count, 1n]);
          clean();
        }
        assert.equal(api.main(), 42);
        assert.equal(api.snapshots(), 1230);
        assert.equal(api.throwUpdate(), 12);
        assert.equal(api.nested(), 18);
        assert.equal(api.structPatterns(), 42);
        for (const [limit, expected] of [[0,0],[3,3],[5,10]]) assert.equal(api.stopAt(limit), expected);
        clean();
      }
    } finally { await rm(directory, {recursive:true, force:true}); }
  });
  test(`${backend}: loop and old field locals survive suspension; null and rejection stop evaluation`, {timeout:60000}, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'walz-high-level-async-'));
    try {
      const {build} = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/high-level-async.walz', directory);
      await instrument(directory, backend);
      const {instantiate} = await import(pathToFileURL(join(directory, 'index.js')));
      const calls = [];
      let rejectAt = 0, memory;
      const options = {eventLoop:{sleep: async value => {
        calls.push(value);
        await Promise.resolve();
        if (value === rejectAt) throw new Error('stop-loop');
      }}, instantiateCore: async (m,i) => {
        const instance = await WebAssembly.instantiate(m,i);
        if (instance.exports.live) memory = instance.exports;
        return instance;
      }};
      const {api, __memory} = await instantiate(options);
      assert.equal(await api.sum([1,2,3]), 6);
      assert.deepEqual(calls, [1,2,3]);
      calls.length = 0;
      rejectAt = 2;
      await assert.rejects(api.sum([1,2,3]), /stop-loop/);
      assert.deepEqual(calls, [1,2]);
      assert.equal(memory.live(), 0);
      assert.ok(Object.values(__memory()).every(n => n === 0));
      // An uncaught host failure poisons a Core instance. Test the independent
      // null-trap path in a fresh instance on both backends.
      const nullRuntime = await instantiate(options);
      calls.length = 0;
      await assert.rejects(nullRuntime.api.nullUpdate(), WebAssembly.RuntimeError);
      assert.deepEqual(calls, []);
      assert.equal(memory.live(), 0);
      assert.ok(Object.values(nullRuntime.__memory()).every(n => n === 0));
    } finally { await rm(directory, {recursive:true, force:true}); }
  });
}
