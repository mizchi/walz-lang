import {test} from 'node:test';
import assert from 'node:assert/strict';
import {build} from './succinct/build.mjs';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instantiate as library} from './succinct/index.mjs';
import {IndexedJS} from './succinct/indexed-js.mjs';

for(const backend of ['core','component'])test(`${backend}: succinct rank/select and wavelet batches agree with independent unpacked oracles`, {timeout:120000}, async () => {
  const dir = await mkdtemp(join(tmpdir(),'wlt-succinct-'));
  try {
    await build(dir,{backend});
    const {instantiate} = await import(pathToFileURL(join(dir,'serial','index.js')));
    const {api} = await instantiate();
    const sdk=await library(pathToFileURL(dir+'/'));
    let seed = 42;
    const random = () => (seed = (Math.imul(seed,1664525)+1013904223)>>>0);
    for (const length of [0,1,31,32,33,511,512,513,2049]) {
      for (const shape of ['zero','one','sparse','random']) {
        const bits = Array.from({length}, (_,i) => shape==='zero'?0:shape==='one'?1:shape==='sparse'?+(i%511===0):random()>>>31);
        const input = new Uint8Array(Math.ceil(length/8));
        bits.forEach((v,i)=>{input[i>>>3]|=v<<(i&7);});
        const index = api.buildBits(input, length);
        const queries=[], expected=[];
        for(let i=0;i<=length;i++) { queries.push(0,i,0,0); expected.push(bits.slice(0,i).reduce((a,b)=>a+b,0)); }
        for(const bit of [0,1]) {
          const positions=bits.flatMap((v,i)=>v===bit?[i]:[]);
          for(let k=0;k<=positions.length;k++) {queries.push(bit?1:2,k,0,0); expected.push(positions[k]??-1);}
        }
        assert.deepEqual([...api.queryMany(index, Int32Array.from(queries))],expected, `${length}:${shape}`);
        assert.deepEqual([...IndexedJS.bits(input,length).queryMany(Int32Array.from(queries))],expected,`JS:${length}:${shape}`);
      }
      const values = Uint8Array.from({length}, ()=>random()>>>24);
      const index=api.buildWavelet(values), queries=[], expected=[];
      for(let i=0;i<80;i++) {
        let l=random()%(length+1),r=random()%(length+1);if(l>r)[l,r]=[r,l];
        const a=random()%257,b=random()%257,lo=Math.min(a,b),hi=Math.max(a,b);
        queries.push(3,l,r,(lo|(hi<<16)));expected.push([...values.slice(l,r)].filter(v=>v>=lo&&v<hi).length);
        const k=r>l?random()%(r-l):0;
        queries.push(4,l,r,k);expected.push([...values.slice(l,r)].sort((a,b)=>a-b)[k]??-1);
      }
      assert.deepEqual([...api.queryMany(index,Int32Array.from(queries))],expected, `wavelet:${length}`);
      assert.deepEqual([...IndexedJS.wavelet(values).queryMany(Int32Array.from(queries))],expected, `JS wavelet:${length}`);
      if(length===2049) {
        for(const workers of [1,2,4]) {
          const frozen=sdk.buildWavelet(values);
          try {
            await frozen.prepare({workers});
            const copy=frozen.snapshot();copy.fill(0);
            const input=values.slice();values.fill(0);
            assert.deepEqual([...await frozen.queryMany(Int32Array.from(queries),{workers})],expected);
            values.set(input);
            const reason=new Error('cancel query');
            const signal=new AbortController();
            const many=new Int32Array(4*200000);
            for(let i=0;i<200000;i++)many.set([4,0,length,i%length],4*i);
            const pending=frozen.queryMany(many,{workers,signal:signal.signal});
            queueMicrotask(()=>signal.abort(reason));
            await assert.rejects(pending,e=>e===reason);
            assert.deepEqual([...await frozen.queryMany(Int32Array.from(queries),{workers})],expected);
          } finally {await frozen.close();await frozen.close();}
          await assert.rejects(frozen.queryMany(new Int32Array()),/closed/);
        }
      }
    }
  } finally {await rm(dir,{recursive:true,force:true});}
});
