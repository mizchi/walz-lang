import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
const exec=promisify(callback),root=fileURLToPath(new URL('..',import.meta.url));
export async function build(source,directory,{mode='js-strings'}={}){
  if(!['utf8','js-strings'].includes(mode))throw new Error('unknown string mode');
  await exec('cargo',['run','--locked','--quiet','--example','js-native-build','--',resolve(source),resolve(directory),mode],{cwd:root,maxBuffer:4*1024*1024});
  return resolve(directory);
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const [source,directory,mode]=process.argv.slice(2);
  if(!source||!directory)throw new Error('usage: build-js-native.mjs source directory [utf8|js-strings]');
  await build(source,directory,{mode});
}
