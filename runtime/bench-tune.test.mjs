import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { verify, compatible, compare } from './bench-tune.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), 'walz-tune-'));
  const source = '(module (@bench "unit" (nop)))';
  // A void batch export, sufficient to test the comparison host's lifecycle.
  const name = Buffer.from('__walz_bench_batch');
  const wasm = new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,1,127,0,3,2,1,0,7,name.length+4,1,name.length,...name,0,0,10,4,1,2,0,11]);
  await writeFile(join(directory, 'source.walz'), source);
  await writeFile(join(directory, 'case-0.wasm'), wasm);
  const driver = await readFile(new URL('./bench-runner.mjs', import.meta.url));
  await writeFile(join(directory, 'driver.mjs'), driver);
  const manifest = {
    schema: 'walz-bench-snapshot-v1',
    cases: [{ name: 'unit', file: 'case-0.wasm' }],
    hashes: { 'source.walz': digest(source), 'case-0.wasm': digest(wasm), 'driver.mjs': digest(driver) },
  };
  await writeFile(join(directory, 'snapshot.json'), JSON.stringify(manifest));
  return { directory, manifest };
}

test('snapshots detect tampering and comparison requires identical inputs and cases', async () => {
  const { directory, manifest } = await fixture();
  try {
    assert.deepEqual(await verify(directory), manifest);
    compatible(manifest, structuredClone(manifest));
    const changed = structuredClone(manifest);
    changed.hashes['source.walz'] = 'changed';
    assert.throws(() => compatible(manifest, changed), /source/);
    changed.hashes['source.walz'] = manifest.hashes['source.walz'];
    changed.cases[0].name = 'different';
    assert.throws(() => compatible(manifest, changed), /cases/);
    await writeFile(join(directory, 'case-0.wasm'), 'modified');
    await assert.rejects(verify(directory), /changed/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('alternating comparison preserves raw samples and never overwrites a report', async () => {
  const a = await fixture(), b = await fixture();
  const output = join(a.directory, 'comparison.json');
  try {
    const report = await compare(a.directory, b.directory, output, { samples: 3, warmup_ms: 0, iterations: 4 });
    assert.equal(report.cases[0].iterations, 4);
    assert.equal(report.cases[0].before_ns.length, 3);
    assert.equal(report.cases[0].after_ns.length, 3);
    assert.deepEqual(report.order, [[0, 1], [1, 0], [0, 1]]);
    await assert.rejects(compare(a.directory, b.directory, output, { samples: 3, warmup_ms: 0, iterations: 4 }), /exist/i);
  } finally {
    await rm(a.directory, { recursive: true, force: true });
    await rm(b.directory, { recursive: true, force: true });
  }
});
