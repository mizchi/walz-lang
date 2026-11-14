// A Rust-built seed starts the build. The published compiler is stage 2,
// compiled by stage 1 in Walz; both stages must reproduce identical bytes.
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,cp,rename} from 'node:fs/promises';
import {resolve,join,relative} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {build as seedBuild} from '../runtime/build-core.mjs';
import {openCompiler,sha256} from '../runtime/selfhost/profiling.mjs';
import {readProject} from '../runtime/selfhost/files.mjs';
import {openStage} from '../runtime/selfhost/staged.mjs';
import {compileWithSelfhost} from '../runtime/selfhost/browser.mjs';
const root=fileURLToPath(new URL('..',import.meta.url));

export async function build(output){
  output=resolve(output);await mkdir(join(root,'target'),{recursive:true});
  const capture=await mkdtemp(join(root,'target/playground-selfhost-build-'));
  console.log('Self-hosted compiler: building the Rust seed');
  const seedPath=await seedBuild(join(root,'examples/selfhost/profile.walz'),join(capture,'seed'));
  const seed=await openCompiler(seedPath);
  const {files,error}=await readProject(seed.api,join(root,'examples/selfhost/staged.walz'));
  assert.ifError(error&&new Error(JSON.stringify(error)));
  const sources=files.map(f=>({path:relative(root,f.path),sha256:sha256(f.source)}));
  await mkdir(join(capture,'inputs'));
  for(let i=0;i<files.length;i++)await writeFile(join(capture,'inputs',i+'.walz'),files[i].source,{flag:'wx'});
  await writeFile(join(capture,'inputs.json'),JSON.stringify({sources,labels:files.map(f=>f.path)},null,2)+'\n',{flag:'wx'});
  const compile=(api,name)=>{
    console.log('Self-hosted compiler: '+name);
    const r=api.compileProject(files);
    assert.equal(r.tag,'ok',name+' '+JSON.stringify(r.tag==='err'?r.val:null));
    assert.ok(WebAssembly.validate(r.val),name+' validates independently');
    return r.val;
  };
  const first=compile(seed.api,'stage 1'),stage1=await openStage(first);
  const second=compile(stage1,'stage 2');await openStage(second);
  assert.deepEqual(second,first,'self-hosted compiler fixed point');
  for(let i=0;i<files.length;i++)assert.equal(sha256(files[i].source),sources[i].sha256);
  await writeFile(join(capture,'stage1.wasm'),first,{flag:'wx'});
  await writeFile(join(capture,'stage2.wasm'),second,{flag:'wx'});
  const api=await openStage(second);
  for(const source of [
    '(module (func $main (export "main") (result i32) (@let $x (type i32) 40 (+ $x 2))))',
    '(module (type $Cell (struct (field $n i32))) (func $main (export "main") (result i32) (struct.get $Cell $n (struct.new $Cell (i32.const 42)))))',
  ]){
    const r=await compileWithSelfhost(api,source);assert.equal(r.status,'success');
    assert.equal(r.sourceView.format,'walz');assert.match(r.sourceView.text,/\(module/);
    assert.equal((await WebAssembly.instantiate(r.wasm)).instance.exports.main(),42);
  }
  assert.equal((await compileWithSelfhost(api,'(module (func $f (result i64) (i32.const 1)))')).status,'error');
  await mkdir(output,{recursive:true});
  await writeFile(join(output,'compiler.wasm.tmp'),second);
  await rename(join(output,'compiler.wasm.tmp'),join(output,'compiler.wasm'));
  for(const [from,to] of [['staged.mjs','bridge.mjs'],['diagnostics.mjs','diagnostics.mjs'],['browser.mjs','browser.mjs']])
    await cp(join(root,'runtime/selfhost',from),join(output,to));
  const report={schema:'walz-playground-selfhost-v1',binaryEquality:true,bytes:second.length,sha256:sha256(second),
    stage1Sha256:sha256(first),sources,seed:seed.artifacts,
    environment:{node:process.version,v8:process.versions.v8,platform:process.platform,arch:process.arch}};
  await writeFile(join(capture,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
  await writeFile(join(output,'build.json'),JSON.stringify(report,null,2)+'\n');
  console.log(`Self-hosted compiler: ${second.length} bytes; fixed point verified; ${relative(root,capture)}`);
  return report;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)
  await build(process.argv[2]??'target/playground/selfhost');
