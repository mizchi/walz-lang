import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createMapper} from './mapper.mjs';

const compiler = resolve('target/debug/wlt');
const source = `;; 日本語 😀\n(module (func $add (export "add-one") (param $n i64) (result i64) (%effect pure) (+ $n 1)))`;

async function fixture(fn) {
  const root = await mkdtemp(join(tmpdir(),'wlt mapper-'));
  const mapper = createMapper();
  try {
    await mapper.handle('initialize',{positionEncodings:['utf-8','utf-16']});
    await fn(root,mapper);
  } finally { mapper.close(); await rm(root,{recursive:true,force:true}); }
}

test('unsaved WLT uses the checked ABI and UTF-8 mappings without writing declarations',async()=>fixture(async(root,mapper)=>{
  const fileName = join(root,'new.wlt');
  await mapper.handle('openProject',{projectHandle:'one',configFileName:join(root,'tsconfig.json'),options:{compiler}});
  const result = await mapper.handle('transform',{projectHandle:'one',fileName,content:source});
  assert.equal(result.extension,'.ts'); assert.deepEqual(result.diagnostics,[]);
  assert.match(result.text,/bigint/); assert.match(result.text,/addOne/);
  assert.ok(result.mappings.length >= 2);
  for(const [start,length,original,size,kind,features] of result.mappings) {
    assert.equal(Buffer.from(source).subarray(original,original+size).toString(),'add-one');
    assert.ok(Buffer.from(result.text).subarray(start,start+length).length);
    assert.equal(kind,1); assert.equal(features & 256,0); // CamelCase exports must not rename WLT identifiers.
  }
  await assert.rejects(readFile(fileName),{code:'ENOENT'});
  await assert.rejects(readFile(join(root,'new.d.wlt.ts')),{code:'ENOENT'});
  const bad = source.replace('(+ $n 1)','$missing');
  const error = await mapper.handle('transform',{projectHandle:'one',fileName,content:bad});
  assert.equal(error.text,'export {};\n'); assert.equal(error.diagnostics.length,1);
  const d=error.diagnostics[0];
  assert.equal(Buffer.from(bad).subarray(d.start,d.start+d.length).toString(),'$missing');
  assert.match(d.messageText,/missing/);
  const recovered=await mapper.handle('transform',{projectHandle:'one',fileName,content:source});
  assert.deepEqual(recovered.diagnostics,[]);
}));

test('project configuration fingerprints dependencies and isolates handles and invalid options',async()=>fixture(async(root,mapper)=>{
  const helper=join(root,'helper.wlt'); await writeFile(helper,source);
  const params={projectHandle:'one',configFileName:join(root,'tsconfig.json'),options:{compiler}};
  const first=await mapper.handle('openProject',params);
  assert.ok(first.watchedFiles.includes(await import('node:fs/promises').then(fs=>fs.realpath(helper))));
  await mapper.handle('closeProject',{projectHandle:'one'});
  await assert.rejects(mapper.handle('transform',{projectHandle:'one',fileName:helper,content:source}),/project/i);
  await writeFile(helper,source.replace('i64','i32'));
  const second=await mapper.handle('openProject',params);
  assert.notEqual(first.configIdentity,second.configIdentity);
  const invalid=await mapper.handle('openProject',{...params,projectHandle:'two',options:{compiler,bindings:5}});
  assert.deepEqual(invalid.optionDiagnostics[0].path,['bindings']);
  const rejected=await mapper.handle('transform',{projectHandle:'two',fileName:helper,content:source});
  assert.match(rejected.diagnostics[0].messageText,/options/i);
  const valid=await mapper.handle('transform',{projectHandle:'one',fileName:helper,content:source});
  assert.deepEqual(valid.diagnostics,[]);
}));

test('static dependency diagnostics name the actual file without assigning its offsets to the root',async()=>fixture(async(root,mapper)=>{
  await writeFile(join(root,'helper.wlt'),'(module (%export (func $answer)) (func $answer (result i32) (%effect pure) $missing))');
  await mapper.handle('openProject',{projectHandle:'one',configFileName:join(root,'tsconfig.json'),options:{compiler}});
  const content='(module (%use "./helper.wlt" (func $answer)) (func $main (export "main") (result i32) (%effect pure) (call $answer)))';
  const result=await mapper.handle('transform',{projectHandle:'one',fileName:join(root,'main.wlt'),content});
  assert.match(result.diagnostics[0].messageText,/helper\.wlt/);
  assert.equal(result.diagnostics[0].start,0);
  assert.equal(result.diagnostics[0].length,0);
}));

test('missing compilers produce a diagnostic and do not leave an unhandled child-process rejection',async()=>fixture(async(root,mapper)=>{
  await mapper.handle('openProject',{projectHandle:'one',configFileName:join(root,'tsconfig.json'),options:{compiler:join(root,'no-wlt-executable')}});
  const result=await mapper.handle('transform',{projectHandle:'one',fileName:join(root,'main.wlt'),content:source});
  assert.match(result.diagnostics[0].messageText,/compiler not found/);
}));
