import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {build} from './build-core.mjs';
import {verifyStages} from './selfhost-stage.mjs';
import {openStage} from './selfhost/staged.mjs';
import {programs,projects,invalid,badProjects} from './selfhost-front/corpus.mjs';
test('self-compiled Core stages reproduce the compiler and the maintained corpus',{timeout:600000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-selfhost-stages-'));
  try{
    const seed=process.env.WALZ_SELFHOST_STAGE_SEED??join(directory,'seed');
    const output=process.env.WALZ_SELFHOST_STAGE_CAPTURE??join(directory,'capture');
    if(!process.env.WALZ_SELFHOST_STAGE_SEED)await build('examples/selfhost/profile.walz',seed);
    const report=await verifyStages(seed,output);
    assert.equal(report.status,'complete');assert.equal(report.binaryEquality,true);
    assert.deepEqual(report.verified,{positive:programs.length+projects.length,negative:invalid.length+badProjects.length});
    assert.equal(report.stages[0].sha256,report.stages[1].sha256);
    assert.equal(report.stages[2].sha256,report.stages[3].sha256);
    const api=await openStage(await readFile(join(output,'stage2.wasm')));
    const source='(module (func $main (export "main") (result i32) (@let $x (type i32) 40 (+ $x 2))))';
    const expanded=api.expandProject([{path:'main.walz',source:new TextEncoder().encode(source)}]);
    assert.equal(expanded.tag,'ok');assert.match(new TextDecoder().decode(expanded.val),/@let/);
    const original=await readFile(join(output,'report.json'));
    await assert.rejects(verifyStages(seed,output),{code:'EEXIST'});
    assert.deepEqual(await readFile(join(output,'report.json')),original,'an existing capture is never replaced');
  }finally{await rm(directory,{recursive:true,force:true});}
});
