import { test, expect } from '@playwright/test';

for (const backend of ['core', 'component']) {
  test(`${backend}: private shared memory stays separate from ABI memory`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const values = await page.evaluate(async () => {
      const { instantiate } = await import(new URL('threads-private/index.js', location.href).href);
      const { api } = await instantiate();
      return [api.add(0, 3), api.add(0, 4), api.load(0), api.packed(32, 255n), api.size(), api.grow(1), api.size()];
    });
    expect(values).toEqual([0, 3, 7, 255n, 1, 1, 2]);
  });
}

test('Core: browser workers and JS Atomics share the host memory and wait queue', async ({ page }) => {
  await page.goto('/core/');
  const result = await page.evaluate(async () => {
    if (!crossOriginIsolated) throw new Error('shared memory requires cross-origin isolation');
    const url = new URL('threads/index.js', location.href).href;
    const { instantiate } = await import(url);
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 2, shared: true });
    const words = new Int32Array(memory.buffer);
    const { api } = await instantiate({ imports: { env: { memory } } });
    const body = `onmessage = async ({ data: { url, memory, wait } }) => {
      try {
        const { instantiate } = await import(url);
        const { api } = await instantiate({ imports: { env: { memory } } });
        if (wait) {
          postMessage('ready');
          postMessage(api.wait32(16, 0, 2000000000n));
        } else {
          for (let i = 0; i < 4000; i++) api.add(0, 1);
          postMessage('done');
        }
      } catch (error) { postMessage({ error: error.message }); }
    };`;
    const blob = URL.createObjectURL(new Blob([body], { type: 'text/javascript' }));
    const all = [];
    const receive = worker => new Promise((resolve, reject) => {
      worker.onmessage = ({ data }) => data?.error ? reject(new Error(data.error)) : resolve(data);
      worker.onerror = reject;
    });
    const start = wait => {
      const worker = new Worker(blob, { type: 'module' });
      all.push(worker);
      const ready = receive(worker);
      worker.postMessage({ url, memory, wait });
      return { worker, ready };
    };
    try {
      const pending = [start(false), start(false)].map(({ ready }) => ready);
      for (let i = 0; i < 4000; i++) Atomics.add(words, 0, 1);
      await Promise.all(pending);
      const count = api.load(0);
      const waiter = start(true);
      if (await waiter.ready !== 'ready') throw new Error('worker did not start');
      const status = receive(waiter.worker);
      let notified = 0;
      for (let i = 0; i < 200 && !notified; i++) {
        notified = Atomics.notify(words, 4, 1);
        if (!notified) await new Promise(resolve => setTimeout(resolve, 5));
      }
      const wasmWait = await status;
      const waiting = Atomics.waitAsync(words, 4, 0, 2000);
      const wasmNotify = api.notify(16, 1);
      const hostWait = await waiting.value;
      return { count, notified, wasmWait, wasmNotify, hostWait };
    } finally {
      for (const worker of all) worker.terminate();
      URL.revokeObjectURL(blob);
    }
  });
  expect(result).toEqual({ count: 12000, notified: 1, wasmWait: 0, wasmNotify: 1, hostWait: 'ok' });
});
