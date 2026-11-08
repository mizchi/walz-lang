import { readFile } from 'node:fs/promises';
import {openHost} from './cli-host.mjs';

const config = JSON.parse(await readFile(process.argv[2], 'utf8'));
const controller = new AbortController();
const interrupt = () => controller.abort('SIGINT');
process.on('SIGINT', interrupt);
process.on('SIGTERM', interrupt);
let session, completed = false;
try {
  if (Number(process.versions.node.split('.')[0]) < 24) throw new Error('Node.js 24+ is required');
  session=await openHost(config,{args:config.args,signal:controller.signal});
  const options=session.options;
  const { instantiate } = await import('./index.js');
  const { start } = await instantiate(options);
  let args = [];
  if (config.arity) {
    if (config.args.length !== config.arity) throw new Error(`expected ${config.arity} export arguments, got ${config.args.length}`);
    args = config.args.map(text => JSON.parse(text, (_, v) => typeof v === 'string' && /^-?\d+n$/.test(v) ? BigInt(v.slice(0, -1)) : v));
  }
  const result = await start(config.entry, args, { signal: controller.signal,...(config.timeoutMs==null?{}:{timeoutMs:config.timeoutMs}) }).result;
  completed = true;
  if (result?.tag === 'err') { console.error(`program error ${result.val}`); process.exitCode = 1; }
  else if (config.exitCode) {
    if (!Number.isInteger(result) || result < 0 || result > 255) throw new Error('--exit-code requires an integer result between 0 and 255');
    process.exitCode = result;
  } else if (!config.stdio && result !== undefined) {
    console.log(typeof result === 'bigint' ? result.toString() : JSON.stringify(result, (_, v) => typeof v === 'bigint' ? `${v}n` : v));
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = error.name === 'TaskTimeoutError' ? 124 : error.name === 'TaskCancelledError' ? 130 : 1;
} finally {
  let cleanupFailed=false;
  try{await session?.close(!completed);}catch{cleanupFailed=true;}
  process.off('SIGINT', interrupt);
  process.off('SIGTERM', interrupt);
  if (cleanupFailed) { console.error('host I/O cleanup failed'); process.exitCode = 1; }
}
