// Shared host initialization for run/test/bench. Each case owns one session.
import {createReadStream,createWriteStream} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {inputCapability,outputCapability} from './node-io.mjs';
import {TEXT_IO,createTextIO} from './text-io.mjs';
import {FILESYSTEM} from './wasi-filesystem.mjs';
import {createNodeFileSystem} from './node-filesystem.mjs';

export async function openHost(config,{signal,args=[]}={}) {
  let host,source,sink,files;
  async function close(failed=false) {
    const results=await Promise.allSettled([
      ()=>source?.return(),()=>sink?.close(failed),()=>files?.close(),()=>host?.[Symbol.asyncDispose]?.(),
    ].map(fn=>Promise.resolve().then(fn)));
    if(results.some(r=>r.status==='rejected'))throw new Error('host I/O cleanup failed');
  }
  try {
    if(config.host){
      const module=await import(pathToFileURL(config.host));
      host=typeof module.default==='function'?await module.default({args,signal}):module.default;
      if(!host||typeof host!=='object')throw new Error('host module must export options or a host factory as default');
    }
    const options={...host,imports:{...host?.imports,'wlt:host/cli@0.1.0':{arguments:()=>args}}};
    if(Object.keys(config.readFiles??{}).length||Object.keys(config.writeFiles??{}).length){
      files=createNodeFileSystem({readFiles:config.readFiles,writeFiles:config.writeFiles});
      options.imports[FILESYSTEM]=files;
    }
    if(config.stdio){
      source=inputCapability(()=>config.input?createReadStream(config.input):process.stdin);
      sink=outputCapability(()=>config.output?createWriteStream(config.output):process.stdout,Boolean(config.output));
      options.imports[TEXT_IO]=createTextIO({input:source,write:sink.write});
    }
    return {options,close};
  } catch(error){await close(true);throw error;}
}
