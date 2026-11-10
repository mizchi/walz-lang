import {test,expect} from '@playwright/test';
import {readFile,writeFile} from 'node:fs/promises';

test('named ESM imports and factories run rich values from a nested base URL', async ({page}, info) => {
  const errors = [], wasm = [];
  page.on('pageerror',error=>errors.push(error.message));
  page.on('response',response=>{if(response.url().endsWith('.wasm'))wasm.push({url:response.url(),status:response.status()});});
  await page.goto('./');
  await expect(page.locator('#answer')).toHaveText('42');
  await expect(page.locator('#text')).toHaveText('Hello, 世界 🌍');
  await expect(page.locator('#bytes')).toHaveText('0, 255, 42');
  await expect(page.locator('#wide')).toHaveText('42');
  await expect(page.locator('#future')).toHaveText('42');
  await page.getByRole('button',{name:'Create two instances'}).click();
  await expect(page.locator('#instances')).toHaveText('84');
  await page.getByRole('button',{name:'Test a Wasm trap'}).click();
  await expect(page.locator('#trap')).toHaveText('RuntimeError');
  expect(errors).toEqual([]);
  if (info.project.name === 'preview') {
    expect(wasm).toHaveLength(2);
    expect(wasm.every(response=>response.status===200 && response.url.includes('/wlt-vite/assets/'))).toBe(true);
  }
});

test('dependency edits reload the dev page and compiler errors recover', async ({page}, info) => {
  test.skip(info.project.name !== 'dev', 'development file watching');
  const helper = 'target/vite-e2e/dev/helper.wlt';
  const original = await readFile(helper,'utf8');
  try {
    await page.goto('./'); await expect(page.locator('#answer')).toHaveText('42');
    await writeFile(helper,original.replace(' 42)', ' 43)'));
    await expect(page.locator('#answer')).toHaveText('43');
    await writeFile(helper,original.replace(' 42)', ' $missing)'));
    await expect(page.locator('vite-error-overlay')).toBeAttached();
    await writeFile(helper,original);
    await expect(page.locator('#answer')).toHaveText('42');
    await expect(page.locator('vite-error-overlay')).not.toBeAttached();
  } finally { await writeFile(helper,original); }
});
