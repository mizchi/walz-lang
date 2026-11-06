// wasm-pack emits a browser ES module. No JavaScript bundler is required.
import { spawnSync } from 'node:child_process';
import { cp, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('..', import.meta.url));
const build = spawnSync('wasm-pack', [
  'build', 'crates/wlt-web', '--target', 'web', '--release',
  '--out-dir', '../../target/playground/pkg', '--out-name', 'wlt_web', '--locked',
], { cwd: root, stdio: 'inherit' });
if (build.error) throw new Error('wasm-pack is required. Install it with: cargo install wasm-pack --locked', { cause: build.error });
if (build.status !== 0) process.exit(build.status ?? 1);
const output = join(root, 'target/playground');
await mkdir(output, { recursive: true });
for (const file of ['index.html', 'style.css', 'main.mjs', 'samples.mjs', 'worker-client.mjs', 'compiler-worker.mjs', 'run-worker.mjs']) {
  await cp(join(root, 'playground', file), join(output, file));
}
await cp(join(root, 'playground/samples'), join(output, 'samples'), { recursive: true });
await mkdir(join(output, 'runtime'), { recursive: true });
for (const name of ['core.mjs', 'host.mjs', 'tasks.mjs', 'resources.mjs']) {
  await cp(join(root, 'runtime', name), join(output, 'runtime', name));
}
console.log(`Playground: ${output}`);
