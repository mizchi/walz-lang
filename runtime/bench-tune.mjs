// Frozen %bench artifacts, unprofiled paired timings, and separate guest profiles.
import { execFile as rawExec } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile, readdir, copyFile, cp, open } from 'node:fs/promises';
import { join, resolve, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { cpus } from 'node:os';
import { Session } from 'node:inspector';
import assert from 'node:assert/strict';
import { summarize } from './profile-summary.mjs';

const exec = promisify(rawExec);
const root = fileURLToPath(new URL('..', import.meta.url));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const save = (path, value) => writeFile(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
const median = xs => {
  const sorted = [...xs].sort((a, b) => a - b);
  return (sorted[(sorted.length - 1) >> 1] + sorted[sorted.length >> 1]) / 2;
};
const host = () => ({ node: process.version, v8: process.versions.v8, platform: process.platform, arch: process.arch, cpu: cpus()[0]?.model });

async function files(directory) {
  const out = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) out.push(...await files(path));
    else out.push(path);
  }
  return out.sort();
}

export async function capture(source, directory) {
  await exec('cargo', ['run', '--locked', '--quiet', '--example', 'bench-capture', '--', resolve(source), resolve(directory)], { cwd: root, timeout: 120000 });
  await copyFile(join(root, 'runtime/bench-runner.mjs'), join(directory, 'driver.mjs'));
  await cp(join(root, 'stdlib'), join(directory, 'stdlib'), { recursive: true });
  const metadata = JSON.parse(await readFile(join(directory, 'metadata.json')));
  const hashes = {};
  for (const file of await files(directory)) hashes[relative(directory, file)] = digest(await readFile(file));
  const snapshot = { ...metadata, date: new Date().toISOString(), host: host(), hashes };
  await save(join(directory, 'snapshot.json'), snapshot);
  return snapshot;
}

export async function verify(directory) {
  const snapshot = JSON.parse(await readFile(join(directory, 'snapshot.json')));
  assert.equal(snapshot.schema, 'wlt-bench-snapshot-v1');
  for (const [file, hash] of Object.entries(snapshot.hashes)) {
    assert.equal(digest(await readFile(join(directory, file))), hash, `snapshot changed: ${file}`);
  }
  return snapshot;
}

export function compatible(before, after) {
  for (const file of ['source.wlt', 'driver.mjs']) {
    assert.equal(before.hashes[file], after.hashes[file], `different benchmark input: ${file}`);
  }
  assert.deepEqual(before.cases, after.cases, 'different benchmark cases');
}

async function instance(directory, file) {
  return (await WebAssembly.instantiate(await readFile(join(directory, file)), {})).instance;
}

export async function compare(before, after, output, options = {}) {
  const snapshots = await Promise.all([verify(before), verify(after)]);
  compatible(...snapshots);
  const { measure } = await import(pathToFileURL(join(resolve(before), 'driver.mjs')));
  const settings = { samples: 15, warmup_ms: 200, sample_ms: 10, ...options };
  assert.ok(Number.isInteger(settings.samples) && settings.samples >= 3 && settings.samples <= 10000);
  // Reserve output before doing expensive work; preserve every completed run.
  const handle = await open(output, 'wx');
  try {
    const order = Array.from({ length: settings.samples }, (_, i) => i % 2 ? [1, 0] : [0, 1]);
    const cases = [];
    for (const c of snapshots[0].cases) {
      const apis = await Promise.all([before, after].map(async directory => (await instance(directory, c.file)).exports));
      const batches = apis.map(api => api.__wlt_bench_batch);
      // Calibrate only the baseline; both variants receive the same count.
      const calibration = measure(batches[0], { ...settings, samples: 1 });
      const iterations = calibration.iterations;
      for (const batch of batches) measure(batch, { ...settings, iterations, samples: 1 });
      const samples = [[], []];
      for (const sequence of order) for (const index of sequence) {
        const result = measure(batches[index], { ...settings, iterations, warmup_ms: 0, samples: 1 });
        samples[index].push(result.samples_ns[0]);
      }
      let scalarCheck = 'not a scalar result';
      if (apis[0].__wlt_bench_sink) {
        try {
          const values = apis.map(api => api.__wlt_bench_sink.value);
          if (values.every(v => typeof v === 'number' || typeof v === 'bigint')) {
            assert.deepEqual(values[0], values[1], `${c.name}: different scalar results`);
            scalarCheck = String(values[0]);
          }
        } catch (error) {
          if (!(error instanceof TypeError)) throw error; // JS cannot expose v128.
        }
      }
      cases.push({ name: c.name, iterations, unchanged_wasm: snapshots[0].hashes[c.file] === snapshots[1].hashes[c.file], before_ns: samples[0], after_ns: samples[1],
        median_before_ns: median(samples[0]), median_after_ns: median(samples[1]),
        median_paired_speedup: median(samples[0].map((n, i) => n / samples[1][i])),
        wasm_bytes: await Promise.all([before, after].map(async d => (await readFile(join(d, c.file))).length)), scalar_check: scalarCheck });
    }
    const report = { schema: 'wlt-bench-comparison-v1', before: resolve(before), after: resolve(after), host: host(), settings, order, cases };
    await handle.writeFile(JSON.stringify(report, null, 2) + '\n');
    return report;
  } finally { await handle.close(); }
}

export async function profile(directory, output, filter = '', iterations = 1000000) {
  assert.ok(Number.isInteger(iterations) && iterations > 0 && iterations <= 0x7fffffff);
  const snapshot = await verify(directory);
  const { measure } = await import(pathToFileURL(join(resolve(directory), 'driver.mjs')));
  const selected = snapshot.cases.filter(c => c.name.includes(filter));
  assert.ok(selected.length, 'no benchmark cases matched');
  await mkdir(resolve(output, '..'), { recursive: true });
  await mkdir(output);
  const summaries = [];
  for (const c of selected) {
    const label = c.file.replace('.wasm', '');
    const batch = (await instance(directory, c.file)).exports.__wlt_bench_batch;
    const { iterations: size } = measure(batch, { samples: 1, warmup_ms: 200, sample_ms: 10 });
    const session = new Session();
    session.connect();
    const post = promisify(session.post.bind(session));
    let cpu;
    try {
      await post('Profiler.enable');
      await post('Profiler.setSamplingInterval', { interval: 1000 });
      await post('Profiler.start');
      const end = performance.now() + 1000;
      while (performance.now() < end) batch(size);
      cpu = (await post('Profiler.stop')).profile;
    } finally { session.disconnect(); }
    await save(join(output, `${label}.cpuprofile`), cpu);
    const counts = new Map();
    for (const id of cpu.samples ?? []) counts.set(id, (counts.get(id) ?? 0) + 1);
    const self = cpu.nodes.map(n => ({ name: n.callFrame.functionName || '(anonymous)', url: n.callFrame.url, samples: counts.get(n.id) ?? 0 }))
      .filter(n => n.samples).sort((a, b) => b.samples - a.samples);
    const guestPath = join(resolve(output), `${label}.guest.json`);
    await exec('wasmtime', ['run', '-W', 'gc=y,function-references=y,exceptions=y', `--profile=guest,${guestPath},1ms`, '--invoke', '__wlt_bench_batch', join(resolve(directory), c.file), String(iterations)], { timeout: 60000, maxBuffer: 1024 * 1024 });
    const guest = summarize(JSON.parse(await readFile(guestPath)));
    summaries.push({ name: c.name, file: c.file, node: { samples: cpu.samples?.length ?? 0, self }, guest });
  }
  const report = { schema: 'wlt-bench-profile-v1', snapshot: resolve(directory), host: host(), wasmtime: (await exec('wasmtime', ['--version'])).stdout.trim(), guest_iterations: iterations, cases: summaries };
  await save(join(output, 'profile.json'), report);
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command, a, b, c, d] = process.argv.slice(2);
  if (command === 'capture' && a && b) {
    const snapshot = await capture(a, b);
    console.log(JSON.stringify({ snapshot: resolve(b), cases: snapshot.cases.length }));
  } else if (command === 'compare' && a && b && c) {
    const report = await compare(a, b, c, { samples: Number(d ?? 15) });
    console.log(JSON.stringify(report.cases.map(({name, median_before_ns, median_after_ns, median_paired_speedup, unchanged_wasm}) => ({name, median_before_ns, median_after_ns, median_paired_speedup, unchanged_wasm})), null, 2));
  } else if (command === 'profile' && a && b) {
    const report = await profile(a, b, c ?? '', Number(d ?? 1000000));
    console.log(JSON.stringify(report.cases.map(entry => ({ name: entry.name, node_samples: entry.node.samples, guest_samples: entry.guest.samples, node_top: entry.node.self.slice(0, 5), guest_top: entry.guest.self.slice(0, 5) })), null, 2));
  }
  else throw new Error('usage: bench-tune.mjs capture source.wlt snapshot | profile snapshot output [filter] [iterations] | compare before after output.json [samples]');
}
