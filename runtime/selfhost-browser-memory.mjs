// Isolated Chromium audit of the deployed worker and UI; never instruments a
// running user preview or modifies deployed files.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, copyFile } from 'node:fs/promises';
import { resolve, dirname, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';
import { assess, limits } from './selfhost-memory/model.mjs';
import { attachWorker, collect, evaluate } from './selfhost-memory/cdp.mjs';
import { types } from './selfhost-front/scaling.mjs';

const sourceRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const settings = { warmupBatches: 2, measuredBatches: 8, callsPerBatch: 128, switches: 12, loadCancellations: 6 };
const hash = data => createHash('sha256').update(data).digest('hex');
const injection = "import {installObserver} from '/__audit/observer.mjs';\nglobalThis.__selfhostAudit=installObserver();\n";
const tracking = () => {
  const NativeWorker = window.Worker;
  let workers = [], buffers = [];
  window.Worker = class extends NativeWorker {
    constructor(...args) {
      super(...args); workers.push(new WeakRef(this));
      this.addEventListener('message', ({ data }) => {
        if (data.result?.wasm) buffers.push(new WeakRef(data.result.wasm.buffer));
        if (data.auditPhase) window.__auditPhase?.(data.auditPhase);
      });
    }
  };
  window.__auditMain = () => {
    workers = workers.filter(ref => ref.deref() !== undefined);
    buffers = buffers.filter(ref => ref.deref() !== undefined);
    return { workers: workers.length, buffers: buffers.length };
  };
};

async function serve(directory) {
  const types = { '.html': 'text/html', '.mjs': 'text/javascript', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' };
  const server = createServer(async (request, response) => {
    try {
      const path = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
      if (path === '/__audit/blank.html') { response.writeHead(200, { 'content-type': 'text/html' }).end('<!doctype html><title>Memory audit</title>'); return; }
      const file = path.startsWith('/__audit/')
        ? resolve(sourceRoot, 'runtime/selfhost-memory', path.slice('/__audit/'.length))
        : resolve(directory, '.' + (path.endsWith('/') ? path + 'index.html' : path));
      if (!file.startsWith(directory + sep) && !file.startsWith(resolve(sourceRoot, 'runtime/selfhost-memory') + sep)) {
        response.writeHead(403).end(); return;
      }
      let data = await readFile(file);
      if (path === '/compiler-worker.mjs') data = Buffer.concat([Buffer.from(injection), data]);
      response.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' }).end(data);
    } catch { response.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise(resolve => server.close(resolve)) };
}

export async function audit(directory, output) {
  directory = resolve(directory); output = resolve(output);
  await mkdir(dirname(output), { recursive: true }); await mkdir(output);
  const binary = await readFile(resolve(directory, 'selfhost/compiler.wasm'));
  const report = { suite: 'selfhost-browser-memory-v3', status: 'running', created: new Date().toISOString(),
    node: process.version, compiler: { bytes: binary.length, sha256: hash(binary) }, settings, limits, samples: [], sources: {} };
  await copyFile(resolve(directory, 'selfhost/compiler.wasm'), resolve(output, 'compiler.wasm'));
  await writeFile(resolve(output, 'worker-prefix.mjs'), injection);
  for (const path of ['runtime/selfhost-browser-memory.mjs', 'runtime/selfhost-memory/cdp.mjs',
    'runtime/selfhost-memory/observer.mjs', 'runtime/selfhost-memory/model.mjs', 'runtime/selfhost-memory/workloads.mjs', 'runtime/selfhost-front/scaling.mjs']) {
    const data = await readFile(resolve(sourceRoot, path)); report.sources[path] = hash(data);
    const destination = resolve(output, 'source', path); await mkdir(dirname(destination), { recursive: true }); await writeFile(destination, data);
  }
  for (const path of ['compiler-worker.mjs', 'worker-client.mjs', 'selfhost-compiler.mjs', 'main.mjs', 'selfhost/bridge.mjs', 'selfhost/browser.mjs', 'selfhost/diagnostics.mjs', 'selfhost/build.json']) {
    const data = await readFile(resolve(directory, path)); report.sources['deployed/' + path] = hash(data);
    const destination = resolve(output, 'deployed', path); await mkdir(dirname(destination), { recursive: true }); await writeFile(destination, data);
  }
  const server = await serve(directory), browser = await chromium.launch();
  report.browser = browser.version();
  const root = await browser.newBrowserCDPSession();
  const targets = async () => (await root.send('Target.getTargets')).targetInfos.filter(t => t.type === 'worker' && t.url.startsWith(server.origin));
  async function waitTargets(count) { await expect.poll(async () => (await targets()).length, { timeout: 15000 }).toBe(count); }
  let worker;
  try {
    const page = await browser.newPage(); await page.addInitScript(tracking);
    await page.goto(server.origin + '/__audit/blank.html');
    const pageSession = await page.context().newCDPSession(page);
    await page.evaluate(async settings => {
      const { WorkerClient } = await import('/worker-client.mjs');
      const { batches } = await import('/__audit/workloads.mjs');
      window.auditInputs = batches(settings.warmupBatches + settings.measuredBatches, settings.callsPerBatch);
      window.auditClient = new WorkerClient('/compiler-worker.mjs', { timeout: 30000 });
      await window.auditClient.run({ engine: 'selfhost', source: window.auditInputs[0][0].source });
    }, settings);
    await writeFile(resolve(output, 'inputs.json'), JSON.stringify(await page.evaluate(() => window.auditInputs)));
    await waitTargets(1); worker = await attachWorker(root, (await targets())[0].targetId);
    const baseline = await collect(worker);
    await evaluate(worker, '__selfhostAudit.pin(32768)'); const pinned = await collect(worker);
    assert.equal(await evaluate(worker, '__selfhostAudit.controlAlive()'), 2);
    await evaluate(worker, '__selfhostAudit.unpin()'); const released = await collect(worker);
    assert.equal(await evaluate(worker, '__selfhostAudit.controlAlive()'), 0);
    assert(pinned.heapUsed - baseline.heapUsed > 1024 * 1024);
    report.control = { baseline, pinned, released, aliveWhilePinned: 2, aliveAfterRelease: 0 };
    for (let batch = 0; batch < settings.warmupBatches + settings.measuredBatches; batch++) {
      const counts = await page.evaluate(async batch => {
        const counts = {};
        for (const w of window.auditInputs[batch]) {
          const response = await window.auditClient.run({ engine: 'selfhost', source: w.source });
          if (response.status !== w.status) throw new Error(w.kind + ': ' + response.status);
          if (w.code && response.diagnostic.code !== `E_SELFHOST_${w.code}`) throw new Error('wrong diagnostic');
          if (response.status === 'success' && (response.exports.length !== 1 || response.runtimeUnavailable)) throw new Error('invalid output');
          counts[w.kind] = (counts[w.kind] ?? 0) + 1;
        }
        return counts;
      }, batch);
      await collect(worker);
      const objects = await evaluate(worker, '__selfhostAudit.drain()');
      assert.deepEqual(objects.live, {});
      const compilerFunctionsAlive = await evaluate(worker, '__selfhostAudit.compilerAlive()');
      assert.equal(compilerFunctionsAlive, 1);
      const memory = await collect(worker); await collect(pageSession);
      const main = await page.evaluate(() => window.__auditMain());
      assert.deepEqual(main, { workers: 1, buffers: 0 });
      if (batch >= settings.warmupBatches) report.samples.push({ ...memory, objects, main, compilerFunctionsAlive, counts });
      console.log(`browser batch ${batch + 1}: heap=${memory.heapUsed}, live=0`);
    }
    report.assessment = assess(report.samples); assert(report.assessment.ok, JSON.stringify(report.assessment));
    await page.evaluate(() => { window.auditClient.cancel(); window.auditClient = null; });
    await waitTargets(0); await worker.close(); worker = null;
    await collect(pageSession);
    report.retired = await page.evaluate(() => window.__auditMain());
    assert.deepEqual(report.retired, { workers: 0, buffers: 0 });

    // Cancel while the real compiler binary is still loading. Reuse the same
    // client and require every retired wrapper and target to disappear.
    report.loadingCancellation = [];
    for (let i = 0; i < settings.loadCancellations; i++) {
      let notify, held;
      const loading = new Promise(resolve => { notify = resolve; });
      await page.route('**/selfhost/compiler.wasm', route => { held = route; notify(); });
      const operation = page.evaluate(async () => {
        const { WorkerClient } = await import('/worker-client.mjs');
        window.auditClient = new WorkerClient('/compiler-worker.mjs', { timeout: 30000 });
        try { await window.auditClient.run({ engine: 'selfhost', source: window.auditInputs[0][0].source }); return 'completed'; }
        catch (error) { return error.message; }
      });
      await loading;
      await page.evaluate(() => { window.auditClient.cancel(); window.auditClient = null; });
      assert.equal(await operation, 'Stopped');
      await held.abort().catch(() => {}); await page.unroute('**/selfhost/compiler.wasm');
      await waitTargets(0); await collect(pageSession);
      const main = await page.evaluate(() => window.__auditMain()); assert.deepEqual(main, { workers: 0, buffers: 0 });
      report.loadingCancellation.push(main);
    }
    await page.close();

    // Exercise actual UI switching, latest-result ownership and run workers.
    const ui = await browser.newPage(); await ui.addInitScript(tracking);
    await ui.goto(server.origin + '/?compiler=selfhost');
    const status = ui.getByTestId('compile-status'); await expect(status).toContainText('Compiled successfully');
    const uiSession = await ui.context().newCDPSession(ui);
    report.switches = [];
    for (let i = 0; i < settings.switches; i++) {
      const engine = i % 2 === 0 ? 'rust' : 'selfhost';
      await ui.getByLabel('Compiler', { exact: true }).selectOption(engine);
      await expect(status).toContainText('Compiled successfully');
      await ui.getByRole('button', { name: 'Run', exact: true }).click();
      await expect(ui.getByTestId('run-output')).toHaveText('42');
      await waitTargets(1); await collect(uiSession);
      const main = await ui.evaluate(() => window.__auditMain());
      assert.deepEqual(main, { workers: 1, buffers: 1 }, 'UI intentionally owns only its current compiler and result');
      report.switches.push({ engine, ...main });
    }
    // Stop 20ms after the native-call entry signal, using the real UI handler.
    // CDP binding notifications can wait for a synchronous Wasm call to finish;
    // normal worker messages let the page respond while that worker is busy.
    await waitTargets(1); worker = await attachWorker(root, (await targets())[0].targetId);
    await evaluate(worker, 'globalThis.__selfhostAuditSignal = phase => self.postMessage({auditPhase:phase})');
    await ui.evaluate(() => {
      window.auditBusyResult = new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('compiler did not signal entry')), 15000);
        let returned = false;
        window.__auditPhase = phase => {
          if (phase === 'done') returned = true;
          if (phase === 'start') setTimeout(() => {
            const returnedBeforeStop = returned;
            document.querySelector('#stop').click();
            clearTimeout(timeout); window.__auditPhase = null;
            resolve({ entered: true, returnedBeforeStop, stopDelayMs: 20 });
          }, 20);
        };
      });
    });
    const busySource = types(768); await writeFile(resolve(output, 'cancelled.walz'), busySource);
    await ui.evaluate(source => {
      const editor = document.querySelector('#source');
      editor.value = source;
      editor.dispatchEvent(new Event('input', { bubbles: true }));
      document.querySelector('#compile').click();
    }, busySource);
    const cancellation = await ui.evaluate(() => window.auditBusyResult);
    await waitTargets(0); await worker.close(); worker = null;
    assert.equal(cancellation.returnedBeforeStop, false, 'the guest must not return before Stop');
    await collect(uiSession); const cancelled = await ui.evaluate(() => window.__auditMain());
    assert.deepEqual(cancelled, { workers: 0, buffers: 0 }); report.busyCancellation = { ...cancellation, ...cancelled };
    await ui.close(); await waitTargets(0);
    report.status = 'passed';
  } catch (error) { report.status = 'failed'; report.error = error.stack; throw error; }
  finally {
    if (worker) await worker.close();
    await browser.close(); await server.close();
    await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  }
  return report;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [, , directory, output] = process.argv;
  if (!directory || !output) throw new Error('usage: node runtime/selfhost-browser-memory.mjs BUILT_PLAYGROUND NEW_OUTPUT');
  const report = await audit(directory, output); console.log(JSON.stringify({ status: report.status, assessment: report.assessment, busyCancellation: report.busyCancellation }, null, 2));
}
