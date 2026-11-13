import {execFile as execCallback} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,copyFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {build} from '../build.mjs';
const execFile=promisify(execCallback);
const root=fileURLToPath(new URL('../../',import.meta.url));
export async function buildWasmUse(output){
  output=resolve(output);
  const source=join(output,'source');await mkdir(source,{recursive:true});
  for(const name of ['main.walz','facade.walz','library.walz','library.wit'])
    await copyFile(join(root,'tests/component-fixtures/wasm-use',name),join(source,name));
  await execFile('cargo',['run','--locked','--quiet','--','build',join(source,'library.walz'),'--target','component','--wit-input',join(source,'library.wit'),'--world','library','-o',join(source,'library.wasm')],{cwd:root,maxBuffer:4*1024*1024});
  await build(join(source,'main.walz'),output);
}
