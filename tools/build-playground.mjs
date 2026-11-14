// wasm-pack emits a browser ES module. No JavaScript bundler is required.
import { spawnSync } from 'node:child_process';
import { cp, mkdir,readFile,writeFile } from 'node:fs/promises';
import {gzipSync,brotliCompressSync,constants} from 'node:zlib';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join, resolve } from 'node:path';
import {build as selfhost} from './build-playground-selfhost.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
export async function build(output = join(root, 'target/playground')) {
  output = resolve(output);
  const built = spawnSync('wasm-pack', [
    'build', 'crates/walz-web', '--target', 'web', '--release',
    '--out-dir', join(output, 'pkg'), '--out-name', 'walz_web', '--locked',
  ], { cwd: root, stdio: 'inherit' });
  if (built.error) throw new Error('wasm-pack is required. Install it with: cargo install wasm-pack --locked', { cause: built.error });
  if (built.status !== 0) throw new Error(`wasm-pack failed (${built.status})`);
  await mkdir(output, { recursive: true });
  await copyAssets(output);
  await selfhost(join(output,'selfhost'));
  const compilers={schema:'walz-playground-compilers-v1'};
  for(const [name,path] of [['rust','pkg/walz_web_bg.wasm'],['selfhost','selfhost/compiler.wasm']]){
    const bytes=await readFile(join(output,path));
    compilers[name]={wasmBytes:bytes.length,gzipBytes:gzipSync(bytes,{level:9}).length,
      brotliBytes:brotliCompressSync(bytes,{params:{[constants.BROTLI_PARAM_QUALITY]:11}}).length};
  }
  await writeFile(join(output,'compiler-info.json'),JSON.stringify(compilers,null,2)+'\n');
  console.log(`Playground: ${output}`);
  return output;
}

export async function copyAssets(output){
  output=resolve(output);
  for (const file of ['index.html', 'style.css', 'main.mjs', 'samples.mjs', 'worker-client.mjs', 'compiler-worker.mjs', 'selfhost-compiler.mjs', 'run-worker.mjs']) {
    await cp(join(root, 'playground', file), join(output, file));
  }
  await cp(join(root, 'playground/samples'), join(output, 'samples'), { recursive: true });
  await mkdir(join(output, 'runtime'), { recursive: true });
  for (const name of ['core.mjs', 'host.mjs', 'tasks.mjs', 'resources.mjs', 'children.mjs', 'readables.mjs']) {
    await cp(join(root, 'runtime', name), join(output, 'runtime', name));
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await build(process.argv[2]);
}
