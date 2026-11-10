import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {join} from 'node:path';

function client(binary){
  const child=spawn(binary,['lsp'],{stdio:['pipe','pipe','pipe']});
  const pending=new Map(),waiters=[],notifications=[],messages=[];let bytes=Buffer.alloc(0),next=0,stderr='';
  child.stderr.on('data',b=>stderr+=b);
  function deliver(message){
    messages.push(message);
    if(message.id!==undefined){pending.get(message.id)?.resolve(message);return;}
    const at=waiters.findIndex(w=>w.match(message));
    if(at>=0)waiters.splice(at,1)[0].resolve(message);else notifications.push(message);
  }
  child.stdout.on('data',b=>{
    bytes=Buffer.concat([bytes,b]);
    for(;;){
      const end=bytes.indexOf('\r\n\r\n');if(end<0)return;
      const length=Number(/Content-Length: (\d+)/i.exec(bytes.subarray(0,end).toString())?.[1]);
      if(!Number.isSafeInteger(length)||length<0||length>16*1024*1024){child.kill();return;}
      if(bytes.length<end+4+length)return;
      deliver(JSON.parse(bytes.subarray(end+4,end+4+length)));bytes=bytes.subarray(end+4+length);
    }
  });
  child.on('error',error=>{for(const p of pending.values())p.reject(error);});
  child.on('exit',code=>{for(const p of pending.values())p.reject(new Error(`LSP exited ${code}: ${stderr}`));});
  function send(message){const text=Buffer.from(JSON.stringify({jsonrpc:'2.0',...message}));child.stdin.write(`Content-Length: ${text.length}\r\n\r\n`);child.stdin.write(text);}
  function request(method,params){
    const id=++next,start=performance.now();
    return new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`LSP timeout: ${method}`));},10000);
      const done=fn=>value=>{clearTimeout(timer);pending.delete(id);fn(value);};
      pending.set(id,{resolve:done(message=>resolve({method,milliseconds:performance.now()-start,...message})),reject:done(reject)});
      send({id,method,params});
    });
  }
  function notification(match){
    const index=notifications.findIndex(match);if(index>=0)return Promise.resolve(notifications.splice(index,1)[0]);
    return new Promise((resolve,reject)=>{
      const waiter={match,resolve:message=>{clearTimeout(timer);resolve(message);}};
      const timer=setTimeout(()=>{const i=waiters.indexOf(waiter);if(i>=0)waiters.splice(i,1);reject(new Error('LSP diagnostics timeout'));},10000);waiters.push(waiter);
    });
  }
  return {child,send,request,notification,messages,stderr:()=>stderr};
}

export async function probe(binary,directory){
  const c=client(binary),observations=[];
  const uri=pathToFileURL(join(directory,'main.walz')).href,library=pathToFileURL(join(directory,'syntax.walz')).href;
  const syntax='(module\n (func $private (param $value i64) (result i64) (@effect pure) (+ $value 1))\n (@macro $next (rule (?value:expr) (call $private ?value)))\n (@macro $with (rule (?name:ident ?value:expr ?body:expr) (bind ?name ?body) (@let ?name ?value ?body)))\n (@export (macro $next) (macro $with)))';
  const text='(module\n (@use "./syntax.walz" (macro $next) (macro $with))\n (func $main (param $seed i64) (result i64) (@effect pure)\n   (; 日本語 🦀 ;) (@$with $amount (+ $seed 40) (+ $amount (@$next (i64.const 1))))))';
  function query(source,needle){const before=source.slice(0,source.lastIndexOf(needle));return {textDocument:{uri},position:{line:before.split('\n').length-1,character:before.split('\n').at(-1).length}};}
  async function observe(name,required,fn){try{const evidence=await fn();observations.push({name,required,achieved:true,evidence});}catch(e){observations.push({name,required,achieved:false,error:e.message});}}
  const wait=(document,version)=>c.notification(m=>m.method==='textDocument/publishDiagnostics'&&m.params.uri===document&&m.params.version===version);
  async function change(document,version,source){c.send({method:'textDocument/didChange',params:{textDocument:{uri:document,version},contentChanges:[{text:source}]}});return (await wait(uri,document===uri?version:1)).params.diagnostics;}
  try{
    const initialized=await c.request('initialize',{});c.send({method:'initialized',params:{}});
    c.send({method:'textDocument/didOpen',params:{textDocument:{uri:library,version:1,text:syntax,languageId:'walz'}}});
    c.send({method:'textDocument/didOpen',params:{textDocument:{uri,version:1,text,languageId:'walz'}}});
    await observe('valid imported binding macro',true,async()=>{assert.deepEqual((await wait(uri,1)).params.diagnostics,[]);return initialized.result.capabilities;});
    await observe('macro signature and cross-file definition',true,async()=>{
      const hover=await c.request('textDocument/hover',query(text,'@$with'));
      const definition=await c.request('textDocument/definition',query(text,'@$with'));
      assert.match(hover.result.contents.value,/macro.*\$with/);assert.equal(definition.result[0].uri,library);
      return {hover,definition};
    });
    await observe('checked argument binding and lexical definition',true,async()=>{
      const hover=await c.request('textDocument/hover',query(text,'$amount (@$next'));
      const definition=await c.request('textDocument/definition',query(text,'$amount (@$next'));
      assert.match(hover.result.contents.value,/i64/);assert.equal(definition.result[0].uri,uri);
      assert.deepEqual(definition.result[0].range.start,query(text,'$amount (+ $seed').position);
      return {hover,definition};
    });
    await observe('expansion exposes original-file origins',true,async()=>{
      const result=await c.request('walz/expandMacro',query(text,'@$with'));
      assert.equal(result.result.schema,'walz-macro-expansion-v1');assert.ok(result.result.expansions.length>0);
      assert.ok(result.result.expansions.every(e=>e.definition_location.location.path.endsWith('syntax.walz')));
      return result;
    });
    const broken=text.replace('(+ $amount (@$next (i64.const 1)))','(+ $amount $missing)');
    await observe('broken balanced body keeps current declared scope',true,async()=>{
      const diagnostics=await change(uri,2,broken);assert.ok(diagnostics.length);
      const completion=await c.request('textDocument/completion',query(broken,'$missing'));
      const hover=await c.request('textDocument/hover',query(broken,'$amount $missing'));
      assert.ok(completion.result.some(item=>item.label==='$amount'));assert.match(hover.result.contents.value,/unverified/);
      return {diagnostics,completion,hover};
    });
    // Restore before changing the dependency; publish carries the root version.
    await change(uri,3,text);
    await observe('unsaved macro dependency invalidates the checked result',true,async()=>{
      const altered=syntax.replace('(@let ?name ?value ?body)','(@let ?name (type i32) ?value ?body)');
      c.send({method:'textDocument/didChange',params:{textDocument:{uri:library,version:2},contentChanges:[{text:altered}]}});
      const diagnostics=(await wait(uri,3)).params.diagnostics;assert.ok(diagnostics.length);
      const expansion=await c.request('walz/expandMacro',query(text,'@$with'));assert.equal(expansion.result.expansions.length,0);
      return {diagnostics,expansion};
    });
    c.send({method:'textDocument/didChange',params:{textDocument:{uri:library,version:3},contentChanges:[{text:syntax}]}});
    await wait(uri,3);
    await observe('unfinished imported macro body still offers its introduced name',false,async()=>{
      const unfinished=text.slice(0,text.lastIndexOf('(+ $amount'))+'(+ $amount ';
      await change(uri,4,unfinished);
      const completion=await c.request('textDocument/completion',query(unfinished,'$amount '));
      assert.ok(completion.result.some(item=>item.label==='$amount'),JSON.stringify(completion.result));return completion;
    });
    await c.request('shutdown',null);c.send({method:'exit'});c.child.stdin.end();
  }finally{if(c.child.exitCode===null)c.child.kill();}
  return {observations,messages:c.messages,stderr:c.stderr()};
}
