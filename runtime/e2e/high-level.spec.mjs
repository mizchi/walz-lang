import {test, expect} from '@playwright/test';
for (const backend of ['core','component']) test(`${backend}: high-level syntax and suspended field updates`, async ({page}) => {
  await page.goto(`/${backend}/`);
  const result = await page.evaluate(async () => {
    const sync = await import(new URL('high-level/index.js', location.href));
    const {api} = await sync.instantiate();
    const async = await import(new URL('high-level-async/index.js', location.href));
    const calls = [];
    const runtime = await async.instantiate({eventLoop:{sleep: async n => { calls.push(n); await Promise.resolve(); }}});
    return {sum:[...api.sum([[10n,1],[30n,1]])].map(String), empty:[...api.sum([])].map(String),
      snapshots:api.snapshots(), stopped:api.stopAt(3), caught:api.throwUpdate(), nested:api.nested(), fields:api.structPatterns(),
      suspended:await runtime.api.sum([1,2,3]), calls};
  });
  expect(result).toEqual({sum:['40','2','1'], empty:['0','0','1'], snapshots:1230, stopped:3, caught:12, nested:18, fields:42,
    suspended:6, calls:[1,2,3]});
});
