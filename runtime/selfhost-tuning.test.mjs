import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {verifyInputs} from './selfhost-tuning.mjs';
import {sha256} from './selfhost/profiling.mjs';

test('tuning snapshots reject changed, extra and escaping input files',async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-tuning-inputs-'));
  try{
    const source='(module)',manifest={schema:'walz-selfhost-tuning-inputs-v1',cases:[{
      name:'tiny',entry:'main.walz',files:[{path:'main.walz',sha256:sha256(source)}],
    }]};
    const record=()=>writeFile(join(directory,'inputs.json'),JSON.stringify(manifest));
    await writeFile(join(directory,'main.walz'),source);await record();
    assert.equal((await verifyInputs(directory)).manifest.cases.length,1);
    await writeFile(join(directory,'main.walz'),'(module (func))');
    await assert.rejects(verifyInputs(directory),/captured input changed/);
    await writeFile(join(directory,'main.walz'),source);
    await writeFile(join(directory,'extra.walz'),source);
    await assert.rejects(verifyInputs(directory),/unexpected input file/);
    await rm(join(directory,'extra.walz'));
    manifest.cases[0].entry='../escape.walz';manifest.cases[0].files[0].path='../escape.walz';await record();
    await assert.rejects(verifyInputs(directory),/inside its snapshot/);
  }finally{await rm(directory,{recursive:true,force:true});}
});
