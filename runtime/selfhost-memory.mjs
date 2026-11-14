// A finite retention audit of an actual self-built Core Wasm compiler.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, copyFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getHeapStatistics, getHeapCodeStatistics } from 'node:v8';
import { collect } from './testing/memory.mjs';
import { openStage } from './selfhost/staged.mjs';
import { compileWithSelfhost } from './selfhost/browser.mjs';
import { installObserver } from './selfhost-memory/observer.mjs';
import { batches } from './selfhost-memory/workloads.mjs';
import { assess, limits } from './selfhost-memory/model.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const encoder = new TextEncoder();
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const settings = Object.freeze({ warmupBatches: 2, measuredBatches: 8, callsPerBatch: 128 });

// Section counts include unexported roots. Validate the binary before calling.
function rootSections(bytes) {
  let offset = 8;
  function leb() {
    let n = 0, shift = 0, byte;
    do { byte = bytes[offset++]; n += (byte & 127) * 2 ** shift; shift += 7; } while (byte & 128);
    return n;
  }
  const result = { tables: 0, memories: 0, globals: 0 };
  while (offset < bytes.length) {
    const section = bytes[offset++], size = leb(), end = offset + size;
    const name = { 4: 'tables', 5: 'memories', 6: 'globals' }[section];
    if (name) result[name] = leb();
    offset = end;
  }
  return result;
}

export async function audit(compiler, output, selfbuildCapture) {
  if (typeof global.gc !== 'function') throw new Error('run with node --expose-gc');
  output = resolve(output);
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output); // Never overwrite a previous capture.
  const binary = await readFile(compiler);
  await WebAssembly.compile(binary);
  const report = {
    suite: 'selfhost-memory-v3', status: 'running', created: new Date().toISOString(),
    environment: { node: process.version, v8: process.versions.v8, platform: process.platform, arch: process.arch },
    compiler: { path: resolve(compiler), bytes: binary.length, sha256: sha256(binary), roots: rootSections(binary) },
    settings, limits, phases: [], sources: {},
  };
  await writeFile(resolve(output, 'compiler.wasm'), binary);
  for (const path of ['runtime/selfhost-memory.mjs', 'runtime/selfhost-memory/model.mjs',
    'runtime/selfhost-memory/workloads.mjs', 'runtime/selfhost-memory/observer.mjs',
    'runtime/selfhost/staged.mjs', 'runtime/selfhost/browser.mjs', 'runtime/selfhost/diagnostics.mjs',
    'runtime/testing/memory.mjs']) {
    const data = await readFile(resolve(root, path));
    report.sources[path] = sha256(data);
    const destination = resolve(output, 'source', path);
    await mkdir(dirname(destination), { recursive: true });
    await copyFile(resolve(root, path), destination);
  }
  const inputs = batches(settings.warmupBatches + settings.measuredBatches, settings.callsPerBatch);
  await writeFile(resolve(output, 'inputs.json'), JSON.stringify(inputs));
  const observer = installObserver();
  let api;
  async function checkpoint() {
    await collect();
    const objects = observer.drain();
    assert.deepEqual(objects.live, {}, 'request GC objects must become unreachable');
    const memory = await collect();
    const heap = getHeapStatistics(), code = getHeapCodeStatistics();
    return { ...memory, objects, nativeContexts: heap.number_of_native_contexts,
      detachedContexts: heap.number_of_detached_contexts, codeAndMetadata: code.code_and_metadata_size };
  }
  async function runCase(w, mode) {
    const source = encoder.encode(w.source);
    if (mode === 'core') {
      const imports = api.imports(source);
      assert(['ok', 'err'].includes(imports.tag));
      if (w.status === 'success') assert.equal(imports.tag, 'ok');
      const result = api.compileProject([{ path: 'playground.walz', source }]);
      assert.equal(result.tag, w.status === 'success' ? 'ok' : 'err', w.kind);
      if (w.code) assert.equal(result.val.code, w.code);
      if (result.tag === 'ok') {
        assert.equal(api.expandProject([{ path: 'playground.walz', source }]).tag, 'ok');
        const instance = await WebAssembly.instantiate(await WebAssembly.compile(result.val));
        assert.equal(String(instance.exports.main()), w.value === '42n' ? '42' : String(w.value));
      }
    } else {
      const response = await compileWithSelfhost(api, w.source);
      assert.equal(response.status, w.status, w.kind);
      if (w.code) assert.equal(response.diagnostic.code, `E_SELFHOST_${w.code}`);
      if (response.status === 'success') {
        const module = await WebAssembly.compile(response.wasm);
        assert.equal(WebAssembly.Module.imports(module).length, 0);
        assert(response.sourceView.text.length > 0);
      }
    }
  }
  try {
    api = await openStage(binary);
    const baseline = await collect();
    observer.pin(32 * 1024);
    const pinned = await collect();
    assert.equal(observer.controlAlive(), 2, 'Wasm fields must keep both byte objects alive');
    observer.unpin();
    const released = await collect();
    assert.equal(observer.controlAlive(), 0, 'removing the Wasm root must release the byte objects');
    assert(pinned.heapUsed - baseline.heapUsed > 1024 * 1024, 'GC heap metric must see the positive control');
    report.control = { logicalBytes: 32 * 1024, baseline, pinned, released, aliveWhilePinned: 2, aliveAfterRelease: 0 };
    for (const mode of ['core', 'playground-response']) {
      const phase = { mode, samples: [], kinds: {} };
      report.phases.push(phase);
      for (let batch = 0; batch < inputs.length; batch++) {
        for (const w of inputs[batch]) {
          await runCase(w, mode);
          phase.kinds[w.kind] = (phase.kinds[w.kind] ?? 0) + 1;
        }
        const sample = await checkpoint();
        if (batch >= settings.warmupBatches) phase.samples.push(sample);
        console.log(`${mode} batch ${batch + 1}/${inputs.length}: heap=${sample.heapUsed}, live=0`);
      }
      phase.assessment = assess(phase.samples);
      assert(phase.assessment.ok, JSON.stringify(phase.assessment));
    }
    if (selfbuildCapture) {
      const manifest = JSON.parse(await readFile(resolve(selfbuildCapture, 'inputs.json'), 'utf8'));
      const files = await Promise.all(manifest.labels.map(async (path, i) => {
        const source = await readFile(resolve(selfbuildCapture, 'inputs', i + '.walz'));
        assert.equal(sha256(source), manifest.sources[i].sha256, 'self-build input provenance');
        const destination = resolve(output, 'selfbuild-inputs', i + '.walz');
        await mkdir(dirname(destination), { recursive: true }); await writeFile(destination, source);
        return { path, source };
      }));
      await writeFile(resolve(output, 'selfbuild-inputs.json'), JSON.stringify(manifest, null, 2) + '\n');
      const phase = { mode: 'self-build', files: files.length, sourceBytes: files.reduce((n, f) => n + f.source.length, 0),
        warmupCalls: 1, measuredCalls: 4, samples: [], outputSha256: report.compiler.sha256 };
      report.phases.push(phase);
      async function compileSelf() {
        const result = api.compileProject(files);
        assert.equal(result.tag, 'ok', 'actual compiler source must compile');
        assert.equal(sha256(result.val), report.compiler.sha256, 'self-build bytes must remain identical');
      }
      for (let i = 0; i < phase.warmupCalls + phase.measuredCalls; i++) {
        console.log(`self-build ${i + 1}/${phase.warmupCalls + phase.measuredCalls}: compiling ${phase.sourceBytes} source bytes`);
        await compileSelf();
        const sample = await checkpoint();
        if (i >= phase.warmupCalls) phase.samples.push(sample);
        console.log(`self-build ${i + 1}: heap=${sample.heapUsed}, live=0`);
      }
      phase.assessment = assess(phase.samples);
      assert(phase.assessment.ok, JSON.stringify(phase.assessment));
    }
    // Retaining an exception is deliberate ownership. Measure it separately,
    // then require release after the last API and exception references go away.
    let errors = [];
    try { api.compileProject([{ path: 'invalid.walz', source: null }]); }
    catch (error) { errors.push(error); }
    observer.failNextCompile();
    try { api.compileProject([{ path: 'trapped.walz', source: new Uint8Array() }]); }
    catch (error) { errors.push(error); }
    assert.equal(errors.length, 2);
    assert(errors[1] instanceof WebAssembly.RuntimeError);
    api = null;
    await collect();
    report.heldErrors = { count: errors.length, compilerFunctionsAlive: observer.compilerAlive(), objects: observer.drain() };
    assert.equal(report.heldErrors.compilerFunctionsAlive, 0, 'held public errors must release their compiler');
    assert.deepEqual(report.heldErrors.objects.live, {});
    errors = null;
    await collect();
    report.teardown = { compilerFunctionsAlive: observer.compilerAlive(), objects: observer.drain(), memory: await collect() };
    assert.equal(report.teardown.compilerFunctionsAlive, 0);
    assert.deepEqual(report.teardown.objects.live, {});
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = error.stack;
    throw error;
  } finally {
    api = null;
    observer.unpin(); observer.restore();
    await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  }
  return report;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [, , compiler, output, selfbuildCapture] = process.argv;
  if (!compiler || !output) throw new Error('usage: node --expose-gc runtime/selfhost-memory.mjs compiler.wasm NEW_OUTPUT [SELFBUILD_CAPTURE]');
  const report = await audit(compiler, output, selfbuildCapture);
  console.log(JSON.stringify({ status: report.status, compiler: report.compiler, phases: report.phases.map(p => ({ mode: p.mode, ...p.assessment })) }, null, 2));
}
