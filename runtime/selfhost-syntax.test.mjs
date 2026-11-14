import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument, collect} from './testing/memory.mjs';
import {programs, invalid, exportName} from './selfhost-front/serialization.mjs';

const bytes=s=>new TextEncoder().encode(s);
const input=source=>[{path:'main.walz',source:bytes(source)}];
const node=(kind,value='',children=[])=>({kind,value:typeof value==='string'?bytes(value):value,children});
for(const backend of ['core','component'])
  test(`${backend}: bounded generated syntax preserves validation and origins`,{timeout:180000},async()=>{
    const directory=await mkdtemp(join(tmpdir(),'walz-selfhost-syntax-'));
    try {
      const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
      async function open(source,name){
        const target=join(directory,name);await build(source,target);await instrument(target,backend);
        let arena;
        const value=await(await import(pathToFileURL(join(target,'index.js')))).instantiate({
          instantiateCore:async(m,imports)=>{
            const instance=await WebAssembly.instantiate(m,imports);
            if(instance.exports.live)arena=instance.exports;
            return instance;
          },
        });
        return {api:value.api,clean(){
          assert.equal(arena.live(),0);
          assert.ok(Object.values(value.__memory()).every(n=>n===0));
        }};
      }
      const fixture=await open('tests/component-fixtures/selfhost-syntax.walz','fixture');
      const atom={value:bytes('i32'),file:1,start:10,end:13};
      const shared=fixture.api.shareAtoms([atom,atom]);fixture.clean();
      assert.equal(shared.tag,'ok');assert.equal(shared.val.nodes,2,'same-origin atoms share one arena node');
      assert.deepEqual(Array.from(shared.val.ids),[0,0]);
      const distinct=fixture.api.shareAtoms([atom,{...atom,file:2},{...atom,start:20,end:23},{...atom,value:bytes('i64')}]);fixture.clean();
      assert.equal(distinct.val.nodes,5,'file, span and value are part of the cache key');
      assert.deepEqual(distinct.val.origins.map(({file,start,end})=>({file,start,end})),[
        {file:1,start:10,end:13},{file:2,start:10,end:13},{file:1,start:20,end:23},{file:1,start:10,end:13},
      ]);
      const keys=Array.from({length:1000},(_,i)=>({...atom,value:bytes('$a'+i.toString().padStart(4,'0')+'z')}));
      const grown=fixture.api.shareAtoms([...keys,...keys]);fixture.clean();
      assert.equal(grown.val.nodes,1001,'growth and colliding origins preserve all distinct spellings');
      assert.deepEqual(Array.from(grown.val.ids),[...keys.map((_,i)=>i),...keys.map((_,i)=>i)]);
      const long={...atom,value:bytes('a'.repeat(65))};
      assert.equal(fixture.api.shareAtoms([long,long]).val.nodes,3,'long atoms are not interned');fixture.clean();
      assert.deepEqual(fixture.api.sharingLimit(1048575),{tag:'ok',val:2});fixture.clean();
      assert.deepEqual(fixture.api.sharingLimit(1048576),{tag:'err',val:{file:7,code:150,start:10,end:13}});
      fixture.clean();
      const assemble=(nodes,check=0)=>{const r=fixture.api.assemble(nodes,check);fixture.clean();return r;};
      assert.deepEqual(assemble([node(0,'module'),node(2,'',[0])]),{tag:'ok',val:2});
      assert.deepEqual(assemble([node(0,'module'),node(2,'',[0])],1),{tag:'ok',val:0});
      for(const nodes of [[],[node(-1)],[node(3,'x')],[node(0,'')],[node(0,'x y')],
        [node(0,'x;y')],[node(0,'{')],[node(0,new Uint8Array([255]))],
        [node(0,'x',[0])],[node(2,'x')],[node(2,'',[0])],
        [node(2,'',[1]),node(0,'x')],[node(2,'',[-1])]]) {
        const r=assemble(nodes);assert.equal(r.tag,'err');assert.equal(r.val.code,nodes.length?3:100);
      }
      const wrongModule=[node(0,'module'),node(0,'bogus'),node(2,'',[1]),node(2,'',[0,2])];
      assert.equal(assemble(wrongModule).tag,'ok');
      assert.deepEqual(assemble(wrongModule,1),{tag:'err',val:{code:101,start:2,end:3}},'assembly is not a type certificate');
      for(const mode of [2,4]){
        assert.deepEqual(assemble([node(0,'module'),node(2,'',[0])],mode),{tag:'ok',val:2});
        assert.equal(assemble([node(2,'',[0])],mode).val.code,3,'indexed nodes still need structural validation');
        assert.deepEqual(assemble(wrongModule,mode+1),{tag:'err',val:{code:101,start:2,end:3}},'wrong index hints are normalized before independent checks');
      }
      const deep=[node(0,'module'),...Array.from({length:128},(_,i)=>node(2,'',[i]))];
      assert.equal(assemble(deep).tag,'ok');
      const tooDeep=assemble([...deep,node(2,'',[128])]);
      assert.equal(tooDeep.tag,'err');assert.equal(tooDeep.val.code,9);
      assert.deepEqual(fixture.api.nodeLimit(262144),{tag:'ok',val:262144});fixture.clean();
      assert.equal(fixture.api.nodeLimit(262145).val.code,8);fixture.clean();
      const payload=new Uint8Array(8388608-4);
      assert.equal(assemble([node(1,payload),node(2,'',[0])]).tag,'ok');
      assert.equal(assemble([node(1,new Uint8Array(8388608-3)),node(2,'',[0])]).val.code,7);
      assert.equal(fixture.api.render(bytes(programs[0][1])).val.code,150,'debug rendering remains separately bounded');fixture.clean();
      const compiler=await open('examples/selfhost/profile.walz','compiler');
      for(const [name,source,expected] of programs){
        const result=compiler.api.compileProject(input(source));
        assert.equal(result.tag,'ok',name+' '+JSON.stringify(result));
        assert.ok(WebAssembly.validate(result.val));
        const {instance}=await WebAssembly.instantiate(result.val);
        assert.equal(instance.exports.f(),expected);
        assert.equal(instance.exports[exportName](),expected);
        compiler.clean();
      }
      const source=invalid[0][1], rejected=compiler.api.profileProject(input(source));
      assert.equal(rejected.result.tag,'err');assert.equal(rejected.result.val.code,110);
      assert.equal(rejected.stats.phase,6);assert.equal(rejected.stats.renderedBytes,0);
      const span=rejected.result.val;
      assert.equal(new TextDecoder().decode(bytes(source).slice(span.start,span.end)),source.slice(source.indexOf('(func'),-1));
      compiler.clean();
      assert.equal(compiler.api.checkProject(input('(module (func (export "f") (result i32) 42))')).tag,'ok');
      compiler.clean();
      await collect();fixture.clean();compiler.clean();
    } finally {await rm(directory,{recursive:true,force:true});}
  });
