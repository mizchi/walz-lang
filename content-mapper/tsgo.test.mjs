import {test} from 'node:test';
import assert from 'node:assert/strict';
import {execFile as callback,spawn} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdtemp,writeFile,mkdir,symlink,rm} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {tmpdir} from 'node:os';
import {frames,frame} from './protocol.mjs';

const execFile=promisify(callback), tsgo=process.env.TSGO;
const source=';; 日本語 😀\n(module (func $add (export "add-one") (param $n i64) (result i64) (%effect pure) (+ $n 1)))';
const consumer='import {addOne} from "./math.wlt";\nconst answer:bigint=addOne(41n);\n';
async function fixture(fn) {
  const root=await mkdtemp(join(tmpdir(),'wlt-tsgo-'));
  try {
    await mkdir(join(root,'node_modules/@mizchi'),{recursive:true});
    await symlink(resolve('.'),join(root,'node_modules/@mizchi/wlt-runtime'),'dir');
    await writeFile(join(root,'tsconfig.json'),JSON.stringify({compilerOptions:{strict:true,noEmit:true,target:'esnext',module:'preserve',types:[]},
      contentMappers:[{package:'@mizchi/wlt-runtime',extensions:['.wlt'],options:{compiler:resolve('target/debug/wlt')}}],include:['*.ts','*.wlt']}));
    await writeFile(join(root,'math.wlt'),source); await writeFile(join(root,'main.ts'),consumer);
    await fn(root);
  } finally { await rm(root,{recursive:true,force:true}); }
}

test('actual TS7 compiler resolves .wlt directly and reports TS and WLT errors without declaration files',{skip:!tsgo},async()=>fixture(async root=>{
  const run=()=>execFile(tsgo,['--runExternalCode','--noEmit','--pretty','false','-p',root],{timeout:30000});
  await run();
  await writeFile(join(root,'main.ts'),consumer.replace('41n','41'));
  const bad=await run().catch(e=>e);
  assert.notEqual(bad.code ?? 0,0); assert.match(bad.stdout,/2345/);
  await writeFile(join(root,'main.ts'),consumer);
  await writeFile(join(root,'math.wlt'),source.replace('(+ $n 1)','$missing'));
  const invalid=await run().catch(e=>e);
  assert.match(invalid.stdout,/wlt9001/); assert.match(invalid.stdout,/math\.wlt/); assert.match(invalid.stdout,/missing/);
  await writeFile(join(root,'math.wlt'),source); await run();
}));

test('actual TS7 LSP maps definitions to WLT exports and sees an unsaved buffer',{skip:!tsgo},async()=>fixture(async root=>{
  const child=spawn(tsgo,['--lsp','--stdio'],{stdio:['pipe','pipe','pipe']});
  let id=0,stderr=''; const pending=new Map(), messages=[];
  child.stderr.on('data',s=>stderr+=s);
  const read=(async()=>{for await(const body of frames(child.stdout)){
    const message=JSON.parse(body); messages.push(message);
    if (message.method && Object.hasOwn(message,'id')) {
      const result=message.method==='workspace/configuration' ? message.params.items.map(()=>({})) : null;
      child.stdin.write(frame({jsonrpc:'2.0',id:message.id,result})); continue;
    }
    const callback=pending.get(message.id);
    if(callback){pending.delete(message.id);message.error?callback.reject(new Error(JSON.stringify(message.error))):callback.resolve(message.result);}
  }})();
  const send=(method,params)=>child.stdin.write(frame({jsonrpc:'2.0',method,params}));
  const call=(method,params)=>new Promise((resolve,reject)=>{
    const requestId=++id;
    const timer=setTimeout(()=>{pending.delete(requestId);reject(new Error(`LSP timed out: ${method}; ${stderr}; ${JSON.stringify(messages)}`));},15000);
    pending.set(requestId,{resolve:value=>{clearTimeout(timer);resolve(value);},reject:error=>{clearTimeout(timer);reject(error);}});
    child.stdin.write(frame({jsonrpc:'2.0',id:requestId,method,params}));
  });
  try {
    await call('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,initializationOptions:{runExternalCode:true},capabilities:{general:{positionEncodings:['utf-16']}}});
    send('initialized',{});
    const uri=pathToFileURL(join(root,'main.ts')).href;
    send('textDocument/didOpen',{textDocument:{uri,languageId:'typescript',version:1,text:consumer}});
    const position={line:1,character:22};
    const hover=await call('textDocument/hover',{textDocument:{uri},position});
    assert.match(JSON.stringify(hover),/bigint/);
    const definition=await call('textDocument/definition',{textDocument:{uri},position});
    assert.match(JSON.stringify(definition),/math\.wlt/);
    assert.ok(JSON.stringify(definition).includes('"line":1'),JSON.stringify(definition));
    const wltUri=pathToFileURL(join(root,'math.wlt')).href;
    send('textDocument/didOpen',{textDocument:{uri:wltUri,languageId:'wlt',version:1,text:source.replaceAll('i64','i32')}});
    const diagnostics=await call('textDocument/diagnostic',{textDocument:{uri}});
    assert.match(JSON.stringify(diagnostics),/bigint|number/);
    await call('shutdown'); send('exit');
  } finally {
    child.kill(); await read;
    for(const callback of pending.values())callback.reject(new Error('LSP closed')); pending.clear();
  }
}));

test('actual TS7 watch invalidates a parent projection after a saved WLT dependency changes',{skip:!tsgo},async()=>fixture(async root=>{
  await writeFile(join(root,'helper.wlt'),source);
  await writeFile(join(root,'math.wlt'),'(module (%include "./helper.wlt"))');
  const child=spawn(tsgo,['--runExternalCode','--noEmit','--watch','--pretty','false','-p',root],{stdio:['ignore','pipe','pipe']});
  let output=''; child.stdout.on('data',s=>output+=s);child.stderr.on('data',s=>output+=s);
  const done=new Promise(resolve=>child.once('exit',resolve));
  async function until(predicate) {
    const deadline=Date.now()+20000;
    while(!predicate(output)) {
      if (child.exitCode !== null || Date.now()>deadline) throw new Error(`TypeScript watch failed: ${output}`);
      await new Promise(resolve=>setTimeout(resolve,25));
    }
  }
  try {
    await until(text=>/Found 0 errors/.test(text));
    output='';
    await writeFile(join(root,'helper.wlt'),source.replaceAll('i64','i32'));
    await until(text=>/2345/.test(text) && /Found [1-9]\d* errors/.test(text));
    assert.match(output,/number|bigint/);
    output='';await writeFile(join(root,'helper.wlt'),source);
    await until(text=>/Found 0 errors/.test(text));
  } finally { child.kill(); await done; }
}));
