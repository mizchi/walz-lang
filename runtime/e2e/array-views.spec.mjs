import { test, expect } from '@playwright/test';

for (const backend of ['core', 'component']) {
  test(`${backend}: generic array views preserve ranges and suspended backing storage`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL('array-views/index.js', location.href).href);
      const { api } = await instantiate();
      const input = [99n, 40n, 2n, 99n];
      return {
        slice: Array.from(api.slice(input, 1, 3, 0, 2), String),
        get: String(api.get(input, 1, 2, 1)),
        empty: Array.from(api.slice([], 0, 0, 0, 0)),
        after: Array.from(await api.after(input, 1, 2, new Promise(resolve => setTimeout(resolve, 1))), String),
      };
    });
    expect(result).toEqual({ slice: ['40', '2'], get: '2', empty: [], after: ['40', '2'] });
  });
}
