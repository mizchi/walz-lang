import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';
import { once } from 'node:events';
import { build } from './build-core.mjs';
import { instantiate as create } from './core.mjs';
import { validate } from './core-contracts.mjs';

const schema = 'wlt-atomic-cells-v1';
const fixture = 'tests/component-fixtures/atomic-cells.wlt';

test('the certified ABI rejects missing or unsupported contracts before loading Wasm', async () => {
  const contract = { hostContract: schema, hostRequired: true, cells: [{ offset: 0, width: 4 }] };
  let loads = 0;
  const options = { atomicHostContract: schema, getCoreModule: () => { loads++; } };
  const memories = [{ module: 'env', name: 'memory', shared: true }];
  for (const invalid of [null, {}, { ...contract, hostContract: 'unknown' },
    { ...contract, hostRequired: undefined }, { ...contract, hostRequired: false },
    { ...contract, cells: null }, { ...contract, cells: [null] },
    { ...contract, cells: [{ offset: 0, width: 2 }] }]) {
    await assert.rejects(create(import.meta.url, 'wlt-core-js-v3', () => {}, options, [], memories, invalid, null, null, validate), /invalid atomic-cell contract/);
  }
  await assert.rejects(create(import.meta.url, 'wlt-core-js-v3', () => {}, options, [], memories, contract), /contract validator is required/);
  await assert.rejects(create(import.meta.url, 'wlt-core-js-v2', () => {}, options, [], memories, contract), /requires Core JS ABI v3/);
  assert.equal(loads, 0);
});
const workerSource = `
const { parentPort, workerData } = require('node:worker_threads');
(async () => {
  const { instantiate } = await import(workerData.url);
  const { api } = await instantiate({ imports: { env: { memory: workerData.memory } }, atomicHostContract: 'wlt-atomic-cells-v1' });
  if (workerData.split) {
    const value = api.load();
    parentPort.postMessage(value);
    parentPort.once('message', () => { api.store(value + 1); parentPort.postMessage('done'); });
  } else {
    for (let i = 0; i < 3000; i++) api.add(1);
    parentPort.postMessage('done');
  }
})().catch(error => { throw error; });`;

test('certified cells require a host contract and share one canonical layout across workers', { timeout: 60000 }, async () => {
  const dir = await mkdtemp(join(tmpdir(), 'wlt-atomic-cells-'));
  const workers = [];
  try {
    await build(fixture, dir);
    const manifest = JSON.parse(await readFile(join(dir, 'manifest.json')));
    assert.equal(manifest.abi, 'wlt-core-js-v3');
    assert.deepEqual(manifest.raceFree.cells, [{ offset: 0, width: 4 }, { offset: 4, width: 4 }, { offset: 8, width: 8 }]);
    const url = pathToFileURL(join(dir, 'index.js')).href;
    const { instantiate } = await import(url);
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1, shared: true });
    let creates = 0;
    await assert.rejects(instantiate({ imports: { env: { memory } }, instantiateCore: () => { creates++; } }), /atomicHostContract/);
    assert.equal(creates, 0);
    await assert.rejects(instantiate({ imports: { env: { memory } }, atomicHostContract: 'wrong-version' }), /atomicHostContract/);
    const { api } = await instantiate({ imports: { env: { memory } }, atomicHostContract: schema });
    const words = new Int32Array(memory.buffer);
    const start = split => {
      const worker = new Worker(workerSource, { eval: true, workerData: { url, memory, split } });
      workers.push(worker);
      return worker;
    };
    const runs = [start(false), start(false)].map(w => once(w, 'message'));
    for (let i = 0; i < 3000; i++) Atomics.add(words, 0, 1);
    await Promise.all(runs);
    assert.equal(api.load(), 9000);
    assert.equal(api.add64(7n), 0n);
    assert.equal(Atomics.add(new BigInt64Array(memory.buffer), 1, 3n), 7n);
    assert.equal(api.add64(0n), 10n);
    assert.equal(api.wait(0n), 2);
    const waiter = Atomics.waitAsync(words, 1, 0, 2000);
    assert.equal(api.notify(1), 1);
    assert.equal(await waiter.value, 'ok');
    api.fence();
    // This accepted, entirely atomic protocol still loses an increment. The
    // certificate proves data-race freedom, not a multi-instruction transaction.
    Atomics.store(words, 0, 0);
    const split = [start(true), start(true)];
    assert.deepEqual(await Promise.all(split.map(w => once(w, 'message'))), [[0], [0]]);
    const done = split.map(w => once(w, 'message'));
    split.forEach(w => w.postMessage('write'));
    await Promise.all(done);
    assert.equal(api.load(), 1);
  } finally {
    await Promise.all(workers.map(w => w.terminate()));
    await rm(dir, { recursive: true, force: true });
  }
});

for (const backend of ['core', 'component']) {
  test(`${backend}: private certified cells need no external host contract`, { timeout: 60000 }, async () => {
    const dir = await mkdtemp(join(tmpdir(), 'wlt-private-atomic-cells-'));
    try {
      const input = join(dir, 'private.wlt');
      await writeFile(input, (await readFile(fixture, 'utf8')).replace('(import "env" "memory" (memory $shared 1 1 shared))', '(memory $shared 1 1 shared)'));
      const { build: compile } = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      const output = join(dir, 'out');
      await compile(input, output);
      const { instantiate } = await import(pathToFileURL(join(output, 'index.js')));
      const { api } = await instantiate();
      assert.equal(api.add(3), 0);
      assert.equal(api.add(4), 3);
      assert.equal(api.load(), 7);
      assert.equal(api.add64(9n), 0n);
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
}
