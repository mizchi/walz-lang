import test from "node:test";
import assert from "node:assert/strict";
import {mkdtemp, rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {pathToFileURL} from "node:url";
import {instrument, collect} from "./testing/memory.mjs";

for (const backend of ["core", "component"])
  test(`${backend}: iterative SCC preserves reachability and dependency order`, {timeout:180000}, async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-selfhost-order-"));
    try {
      const {build} = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/selfhost-order.walz", dir);
      await instrument(dir, backend);
      let arena;
      const value = await (await import(pathToFileURL(join(dir, "index.js")))).instantiate({instantiateCore: async (m, imports) => {
        const instance = await WebAssembly.instantiate(m, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      }});
      const clean = () => {assert.equal(arena.live(),0);assert.ok(Object.values(value.__memory()).every(n=>n===0));};
      function run(n, edges) {
        const r = value.api.scc(n, edges.map(([a])=>a), edges.map(([,b])=>b));
        assert.equal(r.tag,"ok",JSON.stringify(r));assert.equal(r.val.length,n);clean();return Array.from(r.val);
      }
      assert.deepEqual(run(0,[]),[]);
      const chain = run(1024,Array.from({length:1023},(_,i)=>[i,i+1]));
      assert.equal(new Set(chain).size,1024);
      for(let i=0;i<1023;i++)assert.ok(chain[i]>chain[i+1]);
      const ring = run(1024,Array.from({length:1024},(_,i)=>[i,(i+1)%1024]));
      assert.equal(new Set(ring).size,1);
      let seed=17;
      const next=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;};
      for(let sample=0;sample<100;sample++) {
        const n=1+(next()>>>16)%12, edges=[];
        for(let i=0;i<n;i++)for(let j=0;j<n;j++)if((next()>>>16)%4===0)edges.push([i,j]);
        const labels=run(n,edges), reach=Array.from({length:n},(_,i)=>Array.from({length:n},(_,j)=>i===j));
        for(const [a,b] of edges)reach[a][b]=true;
        for(let k=0;k<n;k++)for(let i=0;i<n;i++)for(let j=0;j<n;j++)reach[i][j] ||= reach[i][k] && reach[k][j];
        for(let i=0;i<n;i++)for(let j=0;j<n;j++)assert.equal(labels[i]===labels[j],reach[i][j]&&reach[j][i],JSON.stringify({edges,i,j}));
        for(const [a,b] of edges)assert.ok(labels[a]>=labels[b]);
      }
      for(const [n,from,to] of [[1025,[],[]],[2,[0],[]],[2,[0],[2]],[2,[-1],[0]]]) {
        assert.equal(value.api.scc(n,from,to).tag,"err");clean();
      }
      assert.equal(value.api.scc(1,new Int32Array(65537),new Int32Array(65537)).tag,"err");
      clean();
      const encode = (s) => new TextEncoder().encode(s);
      const independent = value.api.order(encode('(module (type $A (struct (field $B i32))) (type $B (struct (field $A i32))))'));
      assert.equal(independent.tag,"ok");
      assert.ok(!new TextDecoder().decode(independent.val).includes("(rec "), "field binders are not dependency edges");
      clean();
      const mutual = value.api.order(encode('(module (type $A (struct (field (ref null $B)))) (type $B (struct (field (ref null $A)))))'));
      assert.equal(mutual.tag,"ok");
      assert.equal(new TextDecoder().decode(mutual.val).match(/\(rec /g).length,1);
      clean();
      const chainSource = `(module ${Array.from({length:300},(_,i)=>`(type $T${i} (struct ${i===299?'(field i32)':`(field (ref null $T${i+1}))`}))`).join(' ')})`;
      const ordered = value.api.order(encode(chainSource));
      assert.equal(ordered.tag,"ok");
      const names = Array.from(new TextDecoder().decode(ordered.val).matchAll(/\(type \$T(\d+)/g), match=>Number(match[1]));
      assert.deepEqual(names,Array.from({length:300},(_,i)=>299-i));
      clean();
      const parentCycle = value.api.order(encode('(module (type $A (sub $B (struct))) (type $B (sub $A (struct))))'));
      assert.equal(parentCycle.tag,"err");assert.equal(parentCycle.val.code,118);clean();
      await collect();clean();
    } finally {await rm(dir,{recursive:true,force:true});}
  });
