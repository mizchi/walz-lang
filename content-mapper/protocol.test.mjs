import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Readable} from 'node:stream';
import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {frames,frame} from './protocol.mjs';

test('framing handles split and joined UTF-8 messages and rejects invalid lengths and truncation',async()=>{
  const one={id:1,text:'日本語 😀'},two={id:2,text:'next'}, bytes=Buffer.concat([frame(one),frame(two)]);
  const chunks=Array.from({length:bytes.length},(_,i)=>bytes.subarray(i,i+1));
  const results=[];
  for await(const body of frames(Readable.from(chunks))) results.push(JSON.parse(body));
  assert.deepEqual(results,[one,two]);
  for(const invalid of ['Content-Length: 999999999\r\n\r\n','Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}','Content-Length: 3\r\n\r\n{}','x'.repeat(8193)]) {
    await assert.rejects(async()=>{for await(const _ of frames(Readable.from([Buffer.from(invalid)]))){};});
  }
});

test('packaged stdio server implements the upstream project lifecycle without stdout logging',async()=>{
  const child=spawn(process.execPath,[resolve('content-mapper/server.mjs')],{stdio:['pipe','pipe','pipe']});
  const output=[]; let stderr=''; child.stderr.on('data',s=>stderr+=s);
  const done=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',code=>resolve(code));});
  const read=(async()=>{for await(const bytes of frames(child.stdout))output.push(JSON.parse(bytes));})();
  const requests=[
    ['initialize',{positionEncodings:['utf-8']}],
    ['openProject',{projectHandle:'p',configFileName:resolve('examples/vite/tsconfig.json')}],
    ['closeProject',{projectHandle:'p'}],
    ['unknown',{projectHandle:'p'}],
  ];
  child.stdin.end(Buffer.concat(requests.map(([method,params],i)=>frame({jsonrpc:'2.0',id:i,method,params}))));
  assert.equal(await done,0,stderr); await read;
  assert.equal(output.length,4);
  assert.deepEqual(output[0].result,{positionEncoding:'utf-8',diagnosticSource:'wlt'});
  assert.equal(output[2].result,null); assert.equal(output[3].error.code,-32601);
});
