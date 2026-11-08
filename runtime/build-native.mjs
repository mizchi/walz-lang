// Development adapter: a standard WIT Component, transpiled without a WLT SDK.
import {transpileBytes} from '@bytecodealliance/jco-transpile';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const execFile=promisify(callback),root=fileURLToPath(new URL('..',import.meta.url));
export async function buildNative(source,wit,directory,{bindings,world}={}){
 const output=resolve(directory);await mkdir(output,{recursive:true});const binary=join(output,'native.wasm');
 const args=['run','--locked','--quiet','--','build',resolve(source),'--target','component','--wit',resolve(wit),'-o',binary];
 if(bindings)args.push('--bindings',resolve(bindings));if(world)args.push('--world',world);
 await execFile('cargo',args,{cwd:root,maxBuffer:4*1024*1024});
 const generated=await transpileBytes(await readFile(binary),{name:'native',instantiation:'async',asyncMode:'jspi',wasiShim:false,nodejsCompat:false,base64Cutoff:0,noComponentErrorWrapping:false,bindgenEnableWasmExnref:true,multiMemory:true});
 for(const[name,bytes]of Object.entries(generated.files)){
  const path=resolve(output,name);if(!path.startsWith(output+'/'))throw new Error('invalid native artifact path');
  await mkdir(dirname(path),{recursive:true});await writeFile(path,bytes);
 }
 await writeFile(join(output,'package.json'),'{"type":"module"}\n');return output;
}
