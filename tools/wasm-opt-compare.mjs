// Compare frozen Wasm in one process, alternating before/after measurement order.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile, writeFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRunDirectory} from '../evals/macro-usability/support.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const directories = process.argv.slice(2, 4).map(path => resolve(path));
assert.equal(directories.length, 2, 'usage: wasm-opt-compare.mjs BEFORE AFTER NEW_RUN_ID');
const captures = await Promise.all(directories.map(async directory => {
  const manifest = await json(join(directory, 'manifest.json'));
  const result = await json(join(directory, 'result.json'));
  assert.equal(manifest.schema, 'walz-wasm-opt-v1');
  assert.equal(result.passed, true);
  for (const [name, hash] of Object.entries(manifest.inputs)) {
    if (name.startsWith('inputs/')) assert.equal(sha(await readFile(join(directory, name))), hash, name);
  }
  return {manifest, result};
}));
for (const key of ['inputs', 'version', 'profiles', 'features']) {
  assert.deepEqual(captures[0].manifest[key], captures[1].manifest[key], key);
}
assert.deepEqual(captures[0].result.results.map(x => x.name), captures[1].result.results.map(x => x.name));
const output = await createRunDirectory(join(root, 'target/wasm-opt'), process.argv[4]);
const settings = {rounds:15, calls:1000000, warmupCalls:100000};
const results = [];
for (let index = 0; index < captures[0].result.results.length; index++) {
  const rows = captures.map(capture => capture.result.results[index]);
  assert.equal(rows[0].count, rows[1].count);
  assert.deepEqual(rows[0].variants.map(v => v.name), rows[1].variants.map(v => v.name));
  for (let profile = 0; profile < rows[0].variants.length; profile++) {
    const name = rows[0].name, variants = rows.map(row => row.variants[profile]);
    const calls = [];
    for (let side = 0; side < 2; side++) {
      const variant = variants[side];
      const suffix = variant.name === 'raw' ? '' : '.' + variant.name;
      const bytes = await readFile(join(directories[side], 'artifacts', name + suffix + '.wasm'));
      assert.equal(sha(bytes), variant.sha256);
      const {instance} = await WebAssembly.instantiate(bytes);
      assert.equal(instance.exports.main(), 42);
      if (!rows[0].count) continue;
      const e = instance.exports, count = rows[0].count;
      for (let tag = 0; tag < count; tag++) for (const n of [-2147483648, -1, 0, 42, 2147483647]) {
        assert.equal(e.read(e['make' + tag](n)), (n + tag) | 0);
      }
      let state = 20261110;
      const refs = Array.from({length:1024}, () => {
        state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
        const tag = (state >>> 24) % count;
        return e['make' + tag](42 - tag);
      });
      calls.push(iterations => {
        let sum = 0;
        for (let i = 0; i < iterations; i++) sum += e.read(refs[i & 1023]);
        assert.equal(sum, 42 * iterations);
      });
    }
    const row = {name, profile:variants[0].name, variants:variants.map(v => ({
      bytes:v.bytes, codeBytes:v.codeBytes, sha256:v.sha256,
    }))};
    if (calls.length) {
      for (const call of calls) call(settings.warmupCalls);
      row.samplesMs = [[], []];
      for (let round = 0; round < settings.rounds; round++) {
        for (const side of round % 2 ? [1, 0] : [0, 1]) {
          const start = performance.now();
          calls[side](settings.calls);
          row.samplesMs[side].push(performance.now() - start);
        }
      }
      row.medianMs = row.samplesMs.map(median);
      row.medianPairedRatio = median(row.samplesMs[1].map((time, i) => time / row.samplesMs[0][i]));
    }
    results.push(row);
  }
}
const result = {schema:'walz-wasm-opt-comparison-v1', passed:true, createdAt:new Date().toISOString(),
  directories, manifests:captures.map(c => sha(JSON.stringify(c.manifest))), settings,
  node:process.version, v8:process.versions.v8, platform:process.platform, architecture:process.arch,
  toolSha256:sha(await readFile(fileURLToPath(import.meta.url))), results};
await writeFile(join(output, 'comparison.json'), JSON.stringify(result, null, 2) + '\n', {flag:'wx'});
console.log(output);
