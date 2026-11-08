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

test('scoped tasks compile and run with their private scheduler',async({page})=>{
  const source=`(module
    (%closure $Work (%capture $n i32) (result i32) (%effect pure) (+ $n 1))
    (func $main (export "main") (result i32) (%effect host reads writes allocates suspends)
      (%task.scope (result i32)
        (%let $a (%task.spawn i32 $Work (%closure.new $Work 20))
          (%let $b (%task.spawn i32 $Work (%closure.new $Work 20))
            (+ (%task.join i32 (%move $a)) (%task.join i32 (%move $b))))))))`;
  await compile(page,source);
  await expect(page.getByLabel('Export')).toHaveValue('main');
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.getByTestId('run-output')).toHaveText('42');
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

test('an owned unit timer uses the browser readable transport',async({page})=>{
  await compile(page,`(module
    (%use "wlt:core" (type $future) (type $unit))
    (import "wlt:host/event-loop@0.1.0" "after" (func $after (param $ms i32) (result (%inst $future $unit))))
    (func $main (export "main") (result i32) (%effect host suspends)
      (block (result i32) (%future.await $unit (call $after 0)) 42)))`);
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.getByTestId('run-output')).toHaveText('42');
});

test('explicit batch reads compile to ready stream code in the browser',async({page})=>{
  await compile(page,`(module
    (%use "wlt:core" (type $stream) (type $list))
    (func $main (export "main") (result i32) (%effect suspends)
      (%let $s (%stream.cons i32 20 (%stream.cons i32 22 (%stream.empty i32)))
        (%let ($tail $items) (%stream.read-batch i32 8 (%move $s))
          (block (result i32) (%stream.close i32 (%move $tail))
            (+ (%list.get i32 $items 0) (%list.get i32 $items 1)))))))`);
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.getByTestId('run-output')).toHaveText('42');
});
