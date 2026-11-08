import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';
import { once } from 'node:events';
import { build } from './build-core.mjs';
import { instrument } from './testing/memory.mjs';

const fixture = 'tests/component-fixtures/threads.wlt';
const workerSource = `
const { parentPort, workerData } = require('node:worker_threads');
(async () => {
  const { instantiate } = await import(workerData.url);
  const { api } = await instantiate({ imports: { env: { memory: workerData.memory } } });
  if (workerData.wait) {
    parentPort.postMessage('ready');
    const status = workerData.wait === 32 ? api.wait32(16, 0, 2000000000n) : api.wait64(16, 0n, 2000000000n);
    parentPort.postMessage(status);
  } else {
    for (let i = 0; i < workerData.rounds; i++) api.add(0, 1);
    parentPort.postMessage('done');
  }
})().catch(error => { throw error; });`;

// Finite waits and forced worker cleanup keep a failed wakeup from hanging CI.
async function workers(url, memory, count, work) {
  const all = Array.from({ length: count }, () => new Worker(workerSource, { eval: true, workerData: { url, memory, ...work } }));
  try { return await Promise.all(all.map(worker => once(worker, 'message').then(([value]) => value))); }
  finally { await Promise.all(all.map(worker => worker.terminate())); }
}

test('Core: host and workers use one shared Memory, distinct from ABI storage', { timeout: 60000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'wlt-threads-'));
  try {
    await build(fixture, directory);
    await instrument(directory, 'core');
    const url = pathToFileURL(join(directory, 'index.js')).href;
    const { instantiate } = await import(url);
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 2, shared: true });
    const words = new Int32Array(memory.buffer);
    let arena;
    const { api, __memory } = await instantiate({ imports: { env: { memory } }, instantiateCore: async (module, imports) => {
      const instance = await WebAssembly.instantiate(module, imports);
      if (instance.exports.alloc) arena = instance.exports;
      return instance;
    } });
    assert.equal(api.add(0, 3), 0);
    assert.equal(Atomics.add(words, 0, 4), 3);
    assert.equal(api.load(0), 7);
    Atomics.store(words, 0, 0);
    const pending = workers(url, memory, 2, { rounds: 4000 });
    for (let i = 0; i < 4000; i++) Atomics.add(words, 0, 1);
    assert.deepEqual(await pending, ['done', 'done']);
    assert.equal(api.load(0), 12000);
    assert.equal(api.add64(8, 0x100000000n), 0n);
    assert.equal(api.compare64(8, 0x100000000n, -2n), 0x100000000n);
    assert.equal(new BigInt64Array(memory.buffer)[1], -2n);
    assert.equal(api.packed(32, 255n), 255n);
    assert.equal(new Uint8Array(memory.buffer)[32], 0);
    assert.equal(api.wait32(16, 1, 0n), 1);
    assert.equal(api.wait32(16, 0, 0n), 2);
    assert.equal(api.wait64(16, 1n, 0n), 1);
    assert.equal(api.wait64(16, 0n, 0n), 2);
    const waiting = Atomics.waitAsync(words, 4, 0, 2000);
    assert.equal(waiting.async, true);
    assert.equal(api.notify(16, 1), 1);
    assert.equal(await waiting.value, 'ok');
    for (const wait of [32, 64]) {
      const worker = new Worker(workerSource, { eval: true, workerData: { url, memory, wait } });
      try {
        assert.deepEqual(await once(worker, 'message'), ['ready']);
        const result = once(worker, 'message');
        let notified = 0;
        for (let i = 0; i < 200 && !notified; i++) {
          notified = Atomics.notify(words, 4, 1);
          if (!notified) await new Promise(resolve => setTimeout(resolve, 5));
        }
        assert.equal(notified, 1);
        assert.deepEqual(await result, [0]);
      } finally { await worker.terminate(); }
    }
    api.fence();
    assert.equal(api.size(), 1);
    assert.equal(api.grow(1), 1);
    assert.equal(api.size(), 2);
    assert.equal(memory.buffer.byteLength, 131072);
    assert.ok(arena.memory.buffer instanceof ArrayBuffer);
    assert.equal(arena.memory.buffer.byteLength, 65536);
    assert.equal(arena.live(), 0);
    assert.equal(api.grow(1), -1);
    assert.equal(api.load(0), 12000);
    assert.ok(Object.values(__memory()).every(n => n === 0));
    for (const address of [1, 131072]) {
      const trapped = await instantiate({ imports: { env: { memory } } });
      assert.throws(() => trapped.api.load(address), WebAssembly.RuntimeError);
      assert.throws(() => trapped.api.load(0), /after failure/);
      assert.ok(Object.values(trapped.__memory()).every(n => n === 0));
    }
    await assert.rejects(instantiate({ imports: { env: { memory: new WebAssembly.Memory({ initial: 1, maximum: 2 }) } } }), /shared|memory/i);
    await assert.rejects(instantiate({ imports: { env: { memory: new WebAssembly.Memory({ initial: 1, maximum: 3, shared: true }) } } }), /maximum|memory/i);
    await assert.rejects(instantiate(), /memory|env/i);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

for (const backend of ['core', 'component']) {
  test(`${backend}: private shared memory preserves atomic semantics beside ABI memory`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'wlt-private-threads-'));
    try {
      const source = (await readFile(fixture, 'utf8')).replace('(import "env" "memory" (memory $shared 1 2 shared))', '(memory $shared 1 2 shared)');
      const input = join(directory, 'private.wlt');
      await writeFile(input, source);
      const { build: compile } = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      const output = join(directory, 'out');
      await compile(input, output);
      const { instantiate } = await import(pathToFileURL(join(output, 'index.js')));
      const { api } = await instantiate();
      assert.equal(api.add(0, 3), 0);
      assert.equal(api.add(0, 4), 3);
      assert.equal(api.load(0), 7);
      assert.equal(api.add64(8, 9n), 0n);
      assert.equal(api.compare64(8, 9n, -1n), 9n);
      assert.equal(api.packed(32, 255n), 255n);
      assert.equal(api.wait32(16, 0, 0n), 2);
      assert.equal(api.wait64(16, 0n, 0n), 2);
      assert.equal(api.size(), 1);
      assert.equal(api.grow(1), 1);
      assert.equal(api.size(), 2);
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
}
