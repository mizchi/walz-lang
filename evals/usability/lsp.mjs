import {spawn} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {join} from 'node:path';
import assert from 'node:assert/strict';

// A client exercises the public stdio protocol, without using compiler internals.
export async function probeLsp(binary,directory){
 const child=spawn(binary,['lsp'],{stdio:['pipe','pipe','pipe']});
 let buffer=Buffer.alloc(0),sequence=0,stderr='';
 const pending=new Map(),notifications=[],waiters=[];
 const samples=[];
 child.stderr.on('data',bytes=>stderr+=bytes);
 function deliver(message){
  if(message.id!==undefined){pending.get(message.id)?.(message);pending.delete(message.id);return;}
  const i=waiters.findIndex(w=>w.matches(message));
  if(i>=0)waiters.splice(i,1)[0].resolve(message);else notifications.push(message);
 }
 child.stdout.on('data',bytes=>{
  buffer=Buffer.concat([buffer,bytes]);
  for(;;){
   const split=buffer.indexOf('\r\n\r\n');if(split<0)return;
   const size=Number(/Content-Length: (\d+)/i.exec(buffer.subarray(0,split).toString())?.[1]);
   if(!Number.isFinite(size))throw new Error('invalid LSP framing');
   if(buffer.length<split+4+size)return;
   deliver(JSON.parse(buffer.subarray(split+4,split+4+size)));buffer=buffer.subarray(split+4+size);
  }
 });
 function send(message){const bytes=Buffer.from(JSON.stringify({jsonrpc:'2.0',...message}));child.stdin.write(`Content-Length: ${bytes.length}\r\n\r\n`);child.stdin.write(bytes);}
 function request(method,params){
  const id=++sequence,start=performance.now();
  return new Promise((resolve,reject)=>{
   const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`LSP timeout: ${method}`));},10000);
   pending.set(id,message=>{clearTimeout(timer);samples.push({method,milliseconds:performance.now()-start});resolve(message);});send({id,method,params});
  });
 }
 function notification(matches){
  const i=notifications.findIndex(matches);if(i>=0)return Promise.resolve(notifications.splice(i,1)[0]);
  return new Promise((resolve,reject)=>{
   const waiter={matches,resolve:value=>{clearTimeout(timer);resolve(value);}};
   const timer=setTimeout(()=>{const at=waiters.indexOf(waiter);if(at>=0)waiters.splice(at,1);reject(new Error('LSP diagnostics timeout'));},10000);waiters.push(waiter);
  });
 }
 const uri=pathToFileURL(join(directory,'main.walz')).href,dependency=pathToFileURL(join(directory,'calc.walz')).href;
 const text='(module (@use "./calc.walz" (func $sum))\n  (func $main (result i32)\n    (; 日本😀 ;) (@let $answer (call $sum 40 2) $answer)))';
 const position=needle=>{const before=text.slice(0,text.lastIndexOf(needle));return {line:before.split('\n').length-1,character:before.split('\n').at(-1).length};};
 const query=needle=>({textDocument:{uri},position:position(needle)});
 const diagnostics=version=>notification(v=>v.method==='textDocument/publishDiagnostics'&&v.params.uri===uri&&v.params.version===version);
 try{
  const initialized=await request('initialize',{});assert.ok(initialized.result.capabilities.hoverProvider);
  send({method:'initialized',params:{}});
  send({method:'textDocument/didOpen',params:{textDocument:{uri:dependency,version:1,languageId:'walz',text:'(module (@export (func $sum)) (func $sum (param $a i32) (param $b i32) (result i32) (+ $a $b)))'}}});
  send({method:'textDocument/didOpen',params:{textDocument:{uri,version:1,languageId:'walz',text}}});
  assert.deepEqual((await diagnostics(1)).params.diagnostics,[]);
  const hover=await request('textDocument/hover',query('$answer'));
  assert.match(hover.result.contents.value,/i32/);
  const definition=await request('textDocument/definition',query('$sum'));
  assert.equal(definition.result[0].uri,dependency);
  const changed=text.replace('(call $sum 40 2)','(call $sum 40 (i64.const 2))');
  send({method:'textDocument/didChange',params:{textDocument:{uri,version:2},contentChanges:[{text:changed}]}});
  const invalid=(await diagnostics(2)).params.diagnostics;assert.ok(invalid.length>0);
  const badPosition={line:2,character:changed.split('\n')[2].lastIndexOf('$answer')};
  const hoverDuringError=await request('textDocument/hover',{textDocument:{uri},position:badPosition});
  const definitionDuringError=await request('textDocument/definition',query('$sum'));
  assert.ok(hoverDuringError.result);assert.match(hoverDuringError.result.contents.value,/unverified/);
  assert.equal(definitionDuringError.result[0].uri,dependency);
  send({method:'textDocument/didChange',params:{textDocument:{uri,version:3},contentChanges:[{text}]}});
  assert.deepEqual((await diagnostics(3)).params.diagnostics,[]);
  assert.ok((await request('textDocument/hover',query('$answer'))).result);
  const completion=await request('textDocument/completion',query('$answer'));
  assert.ok(completion.result.some(item=>item.label==='$answer'));
  await request('shutdown',null);send({method:'exit'});child.stdin.end();
  const code=await new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error('LSP exit timeout')),5000);child.once('exit',code=>{clearTimeout(timer);resolve(code);});});
  assert.equal(code,0);assert.equal(stderr,'');
  return {capabilities:initialized.result.capabilities,hover:hover.result,definition:definition.result,invalidDiagnostics:invalid,hoverDuringError:hoverDuringError.result,definitionDuringError:definitionDuringError.result,completionItems:completion.result,samples};
 }finally{if(child.exitCode===null)child.kill();}
}
