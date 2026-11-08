import {readFile} from 'node:fs/promises';
import {openHost} from './cli-host.mjs';
import {measureAsync} from './bench-runner.mjs';

const config=JSON.parse(await readFile(process.argv[2],'utf8'));
const controller=new AbortController();
const interrupt=()=>controller.abort('interrupt');
process.on('SIGINT',interrupt);process.on('SIGTERM',interrupt);
const timer=setTimeout(()=>controller.abort('deadline'),config.timeout);
let session,report;
try {
  if(Number(process.versions.node.split('.')[0])<24)throw new Error('Node.js 24+ is required');
  session=await openHost(config,{signal:controller.signal});
  const {instantiate}=await import('./index.js');
  const {start}=await instantiate(session.options);
  const invoke=i=>start(config.entry,config.arity?[i]:[],{signal:controller.signal}).result;
  if(config.mode==='test'){
    const result=await invoke(0);
    report=result===0?{status:'failed',message:'returned 0'}:{status:'passed'};
  } else {
    let sink;
    const batch=async count=>{for(let i=0;i<count;i++)sink=await invoke(i);};
    const measurement=await measureAsync(batch,config.measurement);
    // Keep the last portable result observable, retaining at most one value.
    globalThis.__wlt_bench_sink=sink;
    report={status:'measured',measurement:{...measurement,runtime:`Core+JSPI end-to-end; Node.js ${process.versions.node}; V8 ${process.versions.v8}; ${process.platform}/${process.arch}`}};
  }
} catch(error){report={status:'failed',message:error.stack??String(error)};}
finally {
  try{await session?.close(report?.status==='failed');}
  catch(error){report={status:'failed',message:error.message};}
  clearTimeout(timer);
  process.off('SIGINT',interrupt);process.off('SIGTERM',interrupt);
}
if(controller.signal.aborted && report?.message!=='host I/O cleanup failed')report={status:'timed_out'};
process.stdout.write(JSON.stringify(report));
