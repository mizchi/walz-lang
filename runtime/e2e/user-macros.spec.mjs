import {test, expect} from '@playwright/test';
for (const backend of ['core','component']) test(`${backend}: inferred user syntax runs in Chromium`, async ({page}) => {
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('user-macros/index.js',location.href));
    const {api}=await instantiate();
    return {composed:String(await api.composed(20n,22n)),main:await api.main(),wide:String(await api.wide(21n)),text:await api.text('日本語 🦀'),none:String(await api.unwrap(undefined)),some:String(await api.unwrap(42n)),first:String(await api.first(-9223372036854775808n))};
  });
  expect(result).toEqual({composed:'42',main:42,wide:'42',text:'日本語 🦀',none:'0',some:'42',first:'-9223372036854775808'});
});
