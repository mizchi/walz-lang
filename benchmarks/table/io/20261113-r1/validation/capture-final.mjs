import { readFile, writeFile, mkdir, cp, readdir, copyFile } from 'node:fs/promises';
import { join } from 'node:path';
import { hash, verifySnapshot } from '../../runtime/table-profile-summary.mjs';
const output='benchmarks/table/io/20261113-r1/candidate-final', old='benchmarks/table/20261113-r2';
const original=await verifySnapshot(old);
await mkdir('benchmarks/table/io/20261113-r1',{recursive:true}); await mkdir(output);
for(const name of ['runtime/table','walz/table'])await cp(name,join(output,'source',name),{recursive:true});
for(const name of ['examples/table/kernel.walz','src/regions.rs','Cargo.toml','Cargo.lock','pnpm-lock.yaml','package.json','runtime/table-io-bench.mjs']) {
  const target=join(output,'source',name); await mkdir(join(target,'..'),{recursive:true}); await copyFile(name,target);
}
await mkdir(join(output,'kernel')); await copyFile('target/table-io/primary-final/core.wasm',join(output,'kernel/core.wasm'));
await cp(join(old,'inputs'),join(output,'inputs'),{recursive:true});
async function inventory(dir,prefix='') {
  const out={}; for(const e of await readdir(join(dir,prefix),{withFileTypes:true})) {
    const name=join(prefix,e.name); if(e.isDirectory())Object.assign(out,await inventory(dir,name));
    else out[name]=hash(await readFile(join(dir,name)));
  } return out;
}
const report={schema:'walz-table-artifact-v2',version:2,note:'Artifact capture, not timing results; paired I/O timings are in ../paired/results.json.',
  artifacts:{core:hash(await readFile(join(output,'kernel/core.wasm'))),compiler:hash(await readFile('target/release/deps/walz-c400f193af8f7f13'))},
  compiler:{command:'cargo build --locked --release --bin walz; target/release/walz build examples/table/kernel.walz target/table-io/primary-final/core.wasm',
    profile:'release, normal type/effect/ownership/region analysis enabled',head:'4cb97dfcae8ba104f20ec02e1aca3b701f123c7f'},
  inputs:original.inputs,sources:await inventory(join(output,'source'))};
await writeFile(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');
await verifySnapshot(output); console.log('Captured',output,Object.keys(report.sources).length,'sources');
