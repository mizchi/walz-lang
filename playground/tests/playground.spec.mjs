import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';

async function compile(page, source) {
  await page.getByLabel('WLT source').fill(source);
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
}

test.beforeEach(async ({ page }) => {
  await page.goto('./');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
});

test('loads the Wasm compiler, compiles GC/std syntax, executes i64 and downloads Wasm', async ({ page }) => {
  const requests = [];
  page.on('request', request => requests.push(request.url()));
  await page.reload();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByLabel('Example').selectOption('result');
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await expect(page.getByTestId('wat')).toContainText('struct');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('42n');
  await page.getByRole('tab', { name: 'Analysis', exact: true }).click();
  await expect(page.getByTestId('analysis')).toContainText('module_effects');
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download Wasm', exact: true }).click();
  const artifact = await download;
  expect(artifact.suggestedFilename()).toBe('program.wasm');
  const bytes = await readFile(await artifact.path());
  expect([...bytes.subarray(0, 4)]).toEqual([0, 97, 115, 109]);
  expect(WebAssembly.validate(bytes)).toBe(true);
  expect(requests.some(url => url.endsWith('/pkg/wlt_web_bg.wasm'))).toBe(true);
  expect(requests.every(url => new URL(url).origin === new URL(page.url()).origin)).toBe(true);
});

test('preserves structured diagnostics, invalidates stale output and recovers', async ({ page }) => {
  await compile(page, ';; 日本語🦀\n(module (func $main (result i64) (i32.const 1)))');
  await expect(page.getByTestId('diagnostics')).toContainText('E_TYPE_MISMATCH');
  await expect(page.getByTestId('diagnostics')).toContainText('2:');
  await expect(page.getByRole('button', { name: 'Run', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Download Wasm', exact: true })).toBeDisabled();
  await compile(page, '(module (func (export "main") (result i32) i32.const 7))');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('7');
  await page.getByLabel('WLT source').fill('(module)');
  await expect(page.getByRole('button', { name: 'Run', exact: true })).toBeDisabled();
});

test('timeouts terminate infinite guest execution and a new run still works', async ({ page }) => {
  await compile(page, '(module (func (export "main") (loop $again (br $again))))');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toContainText('Time limit exceeded');
  await page.getByLabel('Example').selectOption('arithmetic');
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('42');
});

test('browser compilation refuses unchecked Lean laws and supports optimization toggle', async ({ page }) => {
  await compile(page, '(module (%law "true" (i32.const 1)))');
  await expect(page.getByTestId('diagnostics')).toContainText('E_HOST_UNAVAILABLE');
  await expect(page.getByTestId('diagnostics')).toContainText('Lean');
  await page.getByLabel('Example').selectOption('arithmetic');
  await page.getByLabel('Optimize').uncheck();
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await expect(page.getByTestId('wat')).toContainText('i32.add');
});

test('guest traps and missing host imports are reported without breaking compilation', async ({ page }) => {
  for (const [source, message] of [
    ['(module (func (export "main") unreachable))', 'unreachable'],
    ['(module (import "host" "f" (func $f)) (func (export "main") (call $f)))', 'host.f'],
    ['(module (func (export "main") (param i32)))', '1 argument'],
  ]) {
    await compile(page, source);
    await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
    await page.getByRole('button', { name: 'Run', exact: true }).click();
    await expect(page.getByTestId('run-output')).toContainText(message);
  }
});

test('stop cancels a running guest and the page stays usable on a narrow screen', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await compile(page, '(module (func (export "main") (loop $again (br $again))))');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('Running…');
  await page.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('Stopped');
  await expect(page.getByRole('button', { name: 'Run', exact: true })).toBeEnabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('Wasm compiler matches the frozen native WAT outputs for all seven frontend fixtures', async ({ page }) => {
  const reference = JSON.parse(await readFile(new URL('../../tests/fixtures/frontend-reference-v4.json', import.meta.url)));
  const fixtures = await Promise.all(Object.keys(reference.cases).map(async name => ({
    name, source: await readFile(new URL(`../../benches/fixtures/frontend-v4/${name}`, import.meta.url), 'utf8'),
  })));
  const actual = await page.evaluate(async fixtures => {
    const { WorkerClient } = await import('./worker-client.mjs');
    const client = new WorkerClient(new URL('./compiler-worker.mjs', location.href), { timeout: 30_000 });
    const hashes = {};
    try {
      for (const { name, source } of fixtures) {
        const result = await client.run({ source, optimize: true });
        if (result.status !== 'success') throw new Error(JSON.stringify(result));
        const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(result.wat));
        hashes[name] = Array.from(new Uint8Array(digest), value => value.toString(16).padStart(2, '0')).join('');
      }
      return hashes;
    } finally { client.cancel(); }
  }, fixtures);
  expect(actual).toEqual(Object.fromEntries(Object.entries(reference.cases).map(([name, output]) => [name, output.wat])));
  await page.getByLabel('Example').selectOption('native');
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('[7, 42n]');
});
