import {once} from 'node:events';
import {createMapper} from './mapper.mjs';
import {frames,frame} from './protocol.mjs';

const mapper=createMapper();
process.once('SIGTERM',()=>{mapper.close();process.exit(0);});
process.once('SIGINT',()=>{mapper.close();process.exit(0);});
try {
  for await (const bytes of frames(process.stdin)) {
    let request,response;
    try {
      try { request=JSON.parse(bytes.toString('utf8')); }
      catch { throw Object.assign(new Error('Parse error'),{code:-32700}); }
      if (!request || request.jsonrpc!=='2.0' || typeof request.method!=='string') throw Object.assign(new Error('Invalid request'),{code:-32600});
      if (!Object.hasOwn(request,'id')) continue;
      const result=await mapper.handle(request.method,request.params);
      response={jsonrpc:'2.0',id:request.id,result};
    } catch(error) {
      response={jsonrpc:'2.0',id:request?.id ?? null,error:{code:Number.isInteger(error.code)?error.code:-32603,message:error.message}};
    }
    if (!process.stdout.write(frame(response))) await once(process.stdout,'drain');
  }
} catch(error) { console.error(`Walz Content Mapper: ${error.message}`); process.exitCode=1; }
finally { mapper.close(); }
