import {build as core} from '../build-core.mjs';
import {resolve} from 'node:path';
import {mkdir, readFile, writeFile, readdir, copyFile} from 'node:fs/promises';
import {promisify} from 'node:util';
import {execFile as execCallback} from 'node:child_process';
import {fileURLToPath,pathToFileURL} from 'node:url';
const execFile=promisify(execCallback);
const root=fileURLToPath(new URL('../../', import.meta.url));
export async function build(directory, {backend='core'}={}) {
  const compiler = backend === 'core' ? core : (await import('../build.mjs')).build;
  await compiler(resolve(root,'examples/succinct/api.wlt'), resolve(directory,'serial'));
  const source=resolve(directory,'source');
  await mkdir(source,{recursive:true});
  for(const name of await readdir(resolve(root,'examples/succinct'))) {
    if(name.endsWith('.wlt')) await copyFile(resolve(root,'examples/succinct',name),resolve(source,name));
  }
  await writeFile(resolve(source,'memory.wlt'), '(module (import "env" "memory" (memory $memory 256 256 shared)) (%export (memory $memory)))');
  await execFile('cargo',['run','--locked','--quiet','--','build',resolve(source,'reader.wlt'),'-o',resolve(directory,'reader.wasm')],{cwd:root,maxBuffer:4*1024*1024});
  const {stdout}=await execFile('cargo',['run','--locked','--quiet','--','race-contracts',resolve(source,'reader.wlt')],{cwd:root,maxBuffer:4*1024*1024});
  await writeFile(resolve(directory,'reader-contract.json'),stdout);
  await mkdir(resolve(directory,'workers'),{recursive:true});
  for(const name of ['host.mjs','agent.mjs'])await copyFile(new URL('../workers/'+name,import.meta.url),resolve(directory,'workers',name));
  const bytes=await readFile(resolve(directory,'reader.wasm'));
  if(!(await WebAssembly.validate(bytes))) throw new Error('invalid reader');
  await mkdir(resolve(directory,'sdk'),{recursive:true});
  for(const name of ['index.mjs','pool.mjs','driver.mjs'])await copyFile(new URL(name,import.meta.url),resolve(directory,'sdk',name));
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  await build(resolve(process.argv[2]??'target/succinct/library'),{backend:process.argv[3]??'core'});
}
