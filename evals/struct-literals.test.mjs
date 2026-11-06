import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {prepare,verify} from './struct-literals/run.mjs';

test('struct literal evaluation freezes counterbalanced packets and rejects tampering',async()=>{
  const root=await mkdtemp(join(tmpdir(),'wlt-struct-eval-'));
  try {
    const run=join(root,'new-parent','run');await prepare(run);
    await assert.rejects(prepare(run),/EEXIST/);
    const manifest=JSON.parse(await readFile(join(run,'manifest.json'),'utf8'));
    assert.equal(manifest.schema,'wlt-struct-readability-v7');
    assert.match(await readFile(join(run,'readerA','packet.md'),'utf8'),/\(%\$User::\{/);
    assert.deepEqual(Object.values(manifest.labels.readerA),Object.values(manifest.labels.readerB).reverse());
    const oracle=JSON.parse(await readFile(join(run,'oracle.json'),'utf8'));
    for(const role of ['readerA','readerB']) {
      const response={variants:Object.fromEntries(Object.keys(manifest.labels[role]).map(label=>[label,{
        answers:oracle.answers,edit:oracle.edits[manifest.labels[role][label]],
        scores:Object.fromEntries(manifest.axes.map(axis=>[axis,3])),evidence:'Q1: field bindings',unclear:[],
      }])),recommendation:'R',limitations:['synthetic reader']};
      await writeFile(join(run,role+'.json'),JSON.stringify(response));
    }
    let summary=await verify(run);assert.equal(summary.correct,56);assert.equal(summary.total,56);
    const path=join(run,'readerA.json'),response=JSON.parse(await readFile(path,'utf8'));
    response.variants.R.answers.user.age=31;await writeFile(path,JSON.stringify(response));
    summary=await verify(run);assert.equal(summary.correct,55);
    response.variants.R.scores.layout=5;await writeFile(path,JSON.stringify(response));
    await assert.rejects(verify(run),/score/);
    await writeFile(join(run,'readerA','packet.md'),'changed');
    await assert.rejects(verify(run),/changed input/);
  }finally{await rm(root,{recursive:true,force:true});}
});
