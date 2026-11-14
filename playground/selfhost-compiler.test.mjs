import test from 'node:test';
import assert from 'node:assert/strict';
import {selfhostDiagnostic,compileWithSelfhost} from '../runtime/selfhost/browser.mjs';

test('self-hosted diagnostics preserve UTF-8 offsets and Unicode columns',()=>{
  const source=';; 日本語🦀\n(module 🦀 $missing)';
  const start=new TextEncoder().encode(source.slice(0,source.indexOf('$missing'))).length;
  const d=selfhostDiagnostic(source,{file:0,code:109,start,end:start+8});
  assert.equal(d.code,'E_SELFHOST_109');
  assert.equal(d.primary.location.path,'playground.walz');
  assert.equal(d.primary.location.line,2);assert.equal(d.primary.location.column,11);
  assert.deepEqual(d.primary.span,{start,end:start+8});
  assert.match(d.message,/unknown local/);
  assert.equal(selfhostDiagnostic(source,{file:-1,code:118,start:0,end:10}).primary,null);
});

test('rejected self-hosted source never expands or returns executable bytes',async()=>{
  let expansions=0;
  const api={compileProject(){return{tag:'err',val:{file:0,code:110,start:0,end:1}};},expandProject(){expansions++;throw new Error('must not expand');}};
  const r=await compileWithSelfhost(api,'x');
  assert.equal(r.status,'error');assert.equal(r.schema,'walz-playground-v1');
  assert.equal(r.diagnostic.code,'E_SELFHOST_110');
  assert.equal(r.wasm,undefined);assert.equal(expansions,0);
});
