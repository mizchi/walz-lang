import {test} from 'node:test';
import assert from 'node:assert/strict';
import {cases,classify,scalingSource} from './macro-usability-v2/cases.mjs';

test('v2 distinguishes wrong execution from an unmet exploratory authoring task',()=>{
  const optional={required:false,expected:{main:'42'}};
  assert.deepEqual(classify(optional,{checks:[{code:1,diagnostic:{code:'E_CHECK'}}],executionCorrect:false}),{passed:true,achieved:false});
  assert.deepEqual(classify(optional,{checks:[{code:0}],executionCorrect:false}),{passed:false,achieved:false});
  assert.deepEqual(classify(optional,{checks:[{code:0}],executionCorrect:true}),{passed:true,achieved:true});
  assert.deepEqual(classify({...optional,required:true},{checks:[{code:1}]}),{passed:false,achieved:false});
  assert.deepEqual(classify({reject:'E_MACRO_BINDING'},{checks:[{code:1,diagnostic:{code:'E_UNKNOWN_LOCAL'}}]}),{passed:false,achieved:false});
  assert.deepEqual(classify({reject:'E_MACRO_BINDING'},{checks:[{code:0}]}),{passed:false,achieved:false});
});
test('v2 scaling holds real call counts constant while changing function distribution',()=>{
  for(const count of [1,32,128,256,512])for(const shape of ['single','separate']){
    const source=scalingSource(count,shape);
    assert.equal((source.match(/\(@\$double /g)??[]).length,count);
    assert.equal((source.match(/\(func \$/g)??[]).length,shape==='single'?1:count+1);
    assert.equal((source.match(/\(export "main"\)/g)??[]).length,1);
  }
  for(const n of [0,513,NaN,1.1])assert.throws(()=>scalingSource(n,'single'));
  assert.throws(()=>scalingSource(32,'unknown'));
});
test('v2 has fixed distinct cases and runtime expectations for accepted positive tasks',()=>{
  assert.equal(new Set(cases.map(x=>x.name)).size,cases.length);
  assert.equal(cases.filter(x=>x.comparison==='option-to-result').length,3);
  for(const item of cases){assert.ok(item.source.startsWith('(module'));assert.ok(item.reject||Object.keys(item.expected).length>0);}
});
test('v2 never counts a crash, timeout or broken check protocol as an unsupported feature',()=>{
  const item={required:false,expected:{main:'42'}};
  for(const record of [{code:101},{code:'timeout'},{code:1},{code:0,protocolValid:false}])
    assert.deepEqual(classify(item,{checks:[record],executionCorrect:false}),{passed:false,achieved:false});
});
test('v2 measures every frozen case including the function variant with no helper definition',async()=>{
  const {describeSyntax}=await import('./macro-usability-v2/cases.mjs');
  const {metrics}=await import('./lib/source.mjs');
  for(const item of cases)assert.ok(describeSyntax(item,metrics).source.tokens>0,item.name);
  assert.equal(describeSyntax(cases.find(x=>x.name==='option-function'),metrics).definitions.tokens,0);
});
