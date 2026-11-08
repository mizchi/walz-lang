import { test, expect } from '@playwright/test';

for (const backend of ['core', 'component']) {
  test(`${backend}: native-certified private cells run in the browser`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const values = await page.evaluate(async () => {
      const { instantiate } = await import(new URL('atomic-cells-private/index.js', location.href).href);
      const { api } = await instantiate();
      return [api.add(3), api.add(4), api.load(), api.add64(9n)];
    });
    expect(values).toEqual([0, 3, 7, 0n]);
  });
}

test('Core: certified browser workers require the explicit host contract', async ({ page }) => {
  await page.goto('/core/');
  const result = await page.evaluate(async () => {
    const url = new URL('atomic-cells/index.js', location.href).href;
    const { instantiate } = await import(url);
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1, shared: true });
    let error;
    try { await instantiate({ imports: { env: { memory } } }); }
    catch (e) { error = e.message; }
    const { api } = await instantiate({ imports: { env: { memory } }, atomicHostContract: 'wlt-atomic-cells-v1' });
    const source = `onmessage = async ({ data: { url, memory } }) => {
      try {
        const { instantiate } = await import(url);
        const { api } = await instantiate({ imports: { env: { memory } }, atomicHostContract: 'wlt-atomic-cells-v1' });
        for (let i = 0; i < 3000; i++) api.add(1);
        postMessage('done');
      } catch (e) { postMessage({ error: e.message }); }
    };`;
    const blob = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    const workers = [new Worker(blob, { type: 'module' }), new Worker(blob, { type: 'module' })];
    try {
      const done = workers.map(w => new Promise((resolve, reject) => {
        w.onmessage = ({ data }) => data?.error ? reject(new Error(data.error)) : resolve(data);
        w.onerror = reject;
        w.postMessage({ url, memory });
      }));
      const words = new Int32Array(memory.buffer);
      for (let i = 0; i < 3000; i++) Atomics.add(words, 0, 1);
      await Promise.all(done);
      return { count: api.load(), error };
    } finally { workers.forEach(w => w.terminate()); URL.revokeObjectURL(blob); }
  });
  expect(result.count).toBe(9000);
  expect(result.error).toContain('atomicHostContract');
});
