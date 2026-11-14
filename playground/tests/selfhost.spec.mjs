import {test,expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';

const hasRust=request=>new URL(request.url()).pathname.includes('/pkg/');
const hasSelfhost=request=>new URL(request.url()).pathname.endsWith('/selfhost/compiler.wasm');
async function compile(page,source){
  await page.getByLabel('Walz source').fill(source);
  await page.getByRole('button',{name:'Compile',exact:true}).click();
}

test('self-hosted compiler loads alone, shows expanded Walz and runs downloadable Wasm',async({page,context})=>{
  await page.setViewportSize({width:390,height:844});
  const requests=[];context.on('request',request=>requests.push(request));
  await page.goto('./?compiler=selfhost');
  await expect(page.getByLabel('Compiler', {exact:true})).toHaveValue('selfhost');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await expect(page.getByLabel('Optimize')).toBeDisabled();
  await expect(page.getByTestId('compiler-description')).toContainText('subset');
  await expect(page.getByRole('tab',{name:'Expanded Walz',exact:true})).toBeVisible();
  await expect(page.getByTestId('wat')).toContainText('@let');
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.getByTestId('run-output')).toHaveText('42');
  await page.getByRole('tab',{name:'Analysis',exact:true}).click();
  await expect(page.getByTestId('analysis')).toContainText('validation-only');
  const pending=page.waitForEvent('download');
  await page.getByRole('button',{name:'Download Wasm',exact:true}).click();
  const download=await pending;
  expect(WebAssembly.validate(await readFile(await download.path()))).toBe(true);
  expect(requests.some(hasSelfhost)).toBe(true);
  expect(requests.some(hasRust)).toBe(false);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});

test('self-hosted errors retain source locations, reject unsupported features and recover',async({page,context})=>{
  const requests=[];context.on('request',request=>requests.push(request));
  await page.goto('./?compiler=selfhost');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await compile(page,';; 日本語🦀\n(module (func $main (result i64) (i32.const 1)))');
  await expect(page.getByTestId('diagnostics')).toContainText('E_SELFHOST_110');
  await expect(page.getByTestId('diagnostics')).toContainText('playground.walz:2:');
  await expect(page.getByRole('button',{name:'Run',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Download Wasm',exact:true})).toBeDisabled();
  await compile(page,'(module (func $main (export "main") (result f64) (f64.const 42)))');
  await expect(page.getByTestId('compile-status')).toContainText('Compilation error');
  await expect(page.getByTestId('diagnostics')).toContainText('E_SELFHOST_');
  await compile(page,`(module
    (@use "walz:core" (type $option))
    (func $value (@type.params $T) (param $x $T) (result $T) $x)
    (func $main (export "main") (result i64)
      (@let $item (type $option<i64>) (@option.some (call $value<i64> 42))
        (@match $item (@case none () (i64.const 0)) (@case some ($v) $v)))))`);
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await expect(page.getByTestId('wat')).toContainText('struct');
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.getByTestId('run-output')).toHaveText('42n');
  expect(requests.some(hasRust)).toBe(false);
});

test('switching compilers cancels loading, preserves source and ignores stale replies',async({page})=>{
  await page.goto('./');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  const source='(module (func $main (export "main") (result i32) (i32.const 7)))';
  await compile(page,source);await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  let loading;const loaded=new Promise(resolve=>{loading=resolve;});
  let release;const held=new Promise(resolve=>{release=resolve;});
  await page.route('**/selfhost/compiler.wasm*',async route=>{loading();await held;await route.continue().catch(()=>{});});
  await page.getByLabel('Compiler',{exact:true}).selectOption('selfhost');
  await loaded;
  await expect(page.getByRole('button',{name:'Run',exact:true})).toBeDisabled();
  await page.getByLabel('Compiler',{exact:true}).selectOption('rust');release();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await expect(page.getByLabel('Walz source')).toHaveValue(source);
  await expect(page.getByLabel('Optimize')).toBeEnabled();
  await page.getByRole('tab',{name:'Analysis',exact:true}).click();
  await expect(page.getByTestId('analysis')).toContainText('module_effects');
  await page.getByRole('button',{name:'Run',exact:true}).click();
  await expect(page.getByTestId('run-output')).toHaveText('7');
});

test('self-hosted load failures do not fall back and can be retried',async({page,context})=>{
  const requests=[];context.on('request',request=>requests.push(request));
  await page.route('**/selfhost/compiler.wasm*',route=>route.abort());
  await page.goto('./?compiler=selfhost');
  await expect(page.getByTestId('compile-status')).toContainText('Could not complete compilation');
  await expect(page.getByLabel('Compiler',{exact:true})).toHaveValue('selfhost');
  expect(requests.some(hasRust)).toBe(false);
  await page.unroute('**/selfhost/compiler.wasm*');
  await page.getByRole('button',{name:'Compile',exact:true}).click();
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  expect(requests.some(hasRust)).toBe(false);
});
