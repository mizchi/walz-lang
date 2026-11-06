import { test, expect } from '@playwright/test';

async function compile(page, source) {
  await page.getByLabel('WLT source').fill(source);
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
}

test.beforeEach(async ({ page }) => {
  await page.goto('./');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
});

test('the host future sample runs through the browser event loop', async ({ page }) => {
  await page.getByLabel('Example', { exact: true }).selectOption('async');
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('42n');
  await page.getByLabel('Optimize').uncheck();
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('42n');
});

test('sleep, yield, streams and fallible futures use the shared JSPI runtime', async ({ page }) => {
  await compile(page, `(module
    (%use "wlt:core" (type $future) (type $stream) (type $tuple) (type $option) (type $result))
    (import "wlt:host/event-loop@0.1.0" "yield" (func $yield))
    (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $ms i32)))
    (import "wlt:host/event-loop@0.1.0" "ticks" (func $ticks (param $n i32) (param $ms i32) (result (%inst $stream i64))))
    (import "wlt:host/event-loop@0.1.0" "fallible-i64" (func $fallible (param $ms i32) (param $code i32) (result (%inst $future (%inst $result i64 i32)))))
    (func $main (export "main") (result i64) (%effect host suspends)
      (block (result i64)
        (call $yield)
        (call $sleep 1)
        (%let ($tail $item) (%stream.next i64 (call $ticks 2 1))
          (block (result i64)
            (%stream.close i64 (%move $tail))
            (%match (%future.await (%inst $result i64 i32) (call $fallible 1 0))
              (case ok ($value) $value)
              (case err ($code) (i64.const -1)))))))
    (func $failure (export "read-error") (result i32) (%effect host suspends)
      (%match (%future.await (%inst $result i64 i32) (call $fallible 1 7))
        (case ok ($value) 0) (case err ($code) $code)))
    (func $argument (export "with-argument") (param $n i32) (result i32) (%effect pure) $n))`);
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('42n');
  await page.getByLabel('Exported function', { exact: true }).selectOption('read-error');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('7');
  await page.getByLabel('Exported function', { exact: true }).selectOption('with-argument');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toContainText('1 argument');
});

test('stopping a pending timer discards its worker and the next run succeeds', async ({ page }) => {
  await compile(page, `(module
    (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $ms i32)))
    (func $main (export "main") (result i32) (%effect host suspends)
      (block (result i32) (call $sleep 60000) 42)))`);
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('Running…');
  await page.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('Stopped');
  await page.getByLabel('Example', { exact: true }).selectOption('async');
  await page.getByRole('button', { name: 'Compile', exact: true }).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(page.getByTestId('run-output')).toHaveText('42n');
});
