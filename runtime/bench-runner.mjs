// The CLI embeds this standalone driver; it has no runtime or npm dependencies.
import { readFile } from 'node:fs/promises';
import { realpathSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const maxIterations = 0x7fffffff;

/** Warm up and time resident Wasm batches. Clock and batch are injectable so
 * calibration and invocation counts can be checked without timing assertions. */
export function measure(batch, options, now = () => performance.now()) {
  let iterations = options.iterations ?? 1;
  const elapsed = (count) => {
    const start = now();
    batch(count);
    return now() - start;
  };
  // Calibrate geometrically, with a hard cap and no integer overflow.
  if (options.iterations == null) {
    while (elapsed(iterations) < options.sample_ms && iterations < maxIterations) {
      iterations = Math.min(iterations * 2, maxIterations);
    }
  }
  const warmupEnd = now() + options.warmup_ms;
  while (now() < warmupEnd) batch(iterations);
  const samples_ns = Array.from({ length: options.samples }, () => elapsed(iterations) * 1e6 / iterations);
  return { iterations, samples_ns };
}

/** Sequential API calls; includes JSPI, codecs and suspension time. */
export async function measureAsync(batch,options,now=()=>performance.now()) {
  let iterations=options.iterations??1;
  const elapsed=async count=>{const start=now();await batch(count);return now()-start;};
  if(options.iterations==null){
    while(await elapsed(iterations)<options.sample_ms && iterations<maxIterations)iterations=Math.min(iterations*2,maxIterations);
  }
  const warmupEnd=now()+options.warmup_ms;
  while(now()<warmupEnd)await batch(iterations);
  const samples_ns=[];
  for(let i=0;i<options.samples;i++)samples_ns.push(await elapsed(iterations)*1e6/iterations);
  return {iterations,samples_ns};
}

async function main() {
  if (Number(process.versions.node.split('.')[0]) < 24) {
    throw new Error('walz bench requires Node.js 24 or later');
  }
  const options = JSON.parse(process.argv[3]);
  const module = await WebAssembly.compile(await readFile(process.argv[2]));
  const instance = await WebAssembly.instantiate(module, {});
  const measurement = measure(instance.exports.__walz_bench_batch, options);
  // Do not read a v128 global through JS: it is a valid observable Wasm value,
  // but the JS API deliberately cannot represent it.
  process.stdout.write(JSON.stringify({
    ...measurement,
    runtime: `Node.js ${process.versions.node}; V8 ${process.versions.v8}; ${process.platform}/${process.arch}`,
  }));
}

if (process.argv[1] && import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  main().catch(error => {
    console.error(error.stack ?? String(error));
    process.exitCode = 1;
  });
}
