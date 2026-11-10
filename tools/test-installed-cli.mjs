// Exercise the distributable package after removing its build-time source tree.
import assert from 'node:assert/strict';
import { execFile as execFileCallback } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, readFile, writeFile, rm, cp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const execFile = promisify(execFileCallback);
const root = fileURLToPath(new URL('..', import.meta.url));
const directory = await mkdtemp(join(tmpdir(), 'walz-installed-cli-'));
const target = resolve(root, 'target/cli-install');
async function run(file, args, { cwd = root, env = process.env, expected = 0 } = {}) {
  const result = await execFile(file, args, { cwd, env, maxBuffer: 16 * 1024 * 1024 })
    .then(output => ({ ...output, code: 0 }), error => {
      if (typeof error.code !== 'number') throw error;
      return error;
    });
  assert.equal(result.code, expected, `${file} ${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
  return result.stdout;
}

try {
  console.log('Packaging the current compiler and embedded assets...');
  const metadata = JSON.parse(await run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1']));
  const pkg = metadata.packages.find(pkg => pkg.name === 'walz');
  assert.ok(pkg);
  await run('cargo', ['package', '-p', 'walz', '--locked', '--allow-dirty', '--no-verify', '--target-dir', join(target, 'package')]);
  const sources = join(directory, 'sources');
  await mkdir(sources);
  // Package archives normalize mtimes. Fresh extraction times invalidate a
  // reused compiler target when embedded sources changed between packages.
  await run('tar', ['-xmzf', join(target, 'package/package', `walz-${pkg.version}.crate`), '-C', sources]);
  const installed = join(directory, 'installed');
  console.log('Installing the packaged crate with Cargo (release profile)...');
  await run('cargo', ['install', '--path', join(sources, `walz-${pkg.version}`), '--locked', '--bin', 'walz', '--root', installed, '--target-dir', join(target, 'build')]);
  const app = join(directory, 'example app');
  await cp(join(sources, `walz-${pkg.version}`, 'examples/cli'), app, { recursive: true });
  const asyncApp=join(directory,'async');await mkdir(asyncApp);
  await cp(join(sources, `walz-${pkg.version}`, 'tests/component-fixtures/scoped-tasks.walz'),join(asyncApp,'main.walz'));
  await rm(sources, { recursive: true });

  const walz = join(installed, 'bin', process.platform === 'win32' ? 'walz.exe' : 'walz');
  // No Cargo, Node, Wasmtime, Lean, pnpm, or source checkout is needed to build.
  const noRuntime = { cwd: app, env: { ...process.env, PATH: '' } };
  assert.equal((await run(walz, ['--version'], noRuntime)).trim(), `walz ${pkg.version}`);
  for (const command of ['new', 'build', 'run', 'test', 'bench']) {
    assert.match(await run(walz, [command, '--help'], noRuntime), /Usage:/);
  }
  // The packaged compiler embeds template/skill assets too. The original
  // checkout is already gone, and generation has no runtime on PATH.
  const generated=join(directory,'generated app');
  await run(walz,['new',generated],{cwd:directory,env:{...process.env,PATH:''}});
  assert.match(await readFile(join(generated,'.agents/skills/walz/SKILL.md'),'utf8'),/name: walz/);
  await run(walz,['build'],{cwd:generated,env:{...process.env,PATH:''}});
  assert.equal((await run(walz,['run'],{cwd:generated})).trim(),'42');
  assert.match(await run(walz,['test'],{cwd:generated}),/2 passed; 0 failed; 0 errors/);
  const generatedReport=JSON.parse(await run(walz,['bench','--json','--samples','1','--iterations','2','--warmup-ms','0'],{cwd:generated}));
  assert.equal(generatedReport.benchmarks[0].outcome.status,'measured');
  console.log('Checking build, test and bench outside the source checkout...');
  await run(walz, ['build'], noRuntime);
  assert.deepEqual([...(await readFile(join(app, 'target/main.wasm'))).subarray(0, 4)], [0, 97, 115, 109]);
  assert.equal((await run('wasmtime', ['run', '-W', 'gc=y,function-references=y,exceptions=y', '--invoke', 'main', 'target/main.wasm'], { cwd: app })).trim(), '42');
  assert.equal((await run(walz,['run'],{cwd:app})).trim(),'42');
  assert.equal((await run(walz,['run','--target','core-js'],{cwd:asyncApp})).trim(),'42');
  await run(walz,['build','--target','core-js'],{cwd:asyncApp});
  await rm(join(asyncApp,'main.walz'));
  assert.equal((await run(process.execPath,['--input-type=module','-e',"const {instantiate,withTaskScope}=await import('./target/main-js/index.js');const {start}=await instantiate();console.log(await withTaskScope(start,async scope=>scope.start('main').result,{timeoutMs:1000}));"],{cwd:asyncApp})).trim(),'42');
  await writeFile(join(asyncApp,'timer.test.walz'),`(module
    (@use "walz:core" (type $future) (type $unit))
    (import "walz:host/event-loop@0.1.0" "after" (func $after (param $ms i32) (result (@inst $future $unit))))
    (func $work (result i32) (@effect host suspends) (block (result i32) (@future.await $unit (call $after 0)) 42))
    (@test "timer" (= (call $work) 42))
    (@bench "timer" (result i32) (call $work)))`);
  assert.match(await run(walz,['test','timer.test.walz','--target','core-js'],{cwd:asyncApp}),/1 passed; 0 failed; 0 errors/);
  const asyncReport=JSON.parse(await run(walz,['bench','timer.test.walz','--target','core-js','--json','--samples','2','--iterations','2','--warmup-ms','0'],{cwd:asyncApp}));
  assert.equal(asyncReport.benchmarks[0].outcome.status,'measured');
  assert.match(await run(walz, ['test'], { cwd: app }), /1 passed; 0 failed; 0 errors/);
  const report = JSON.parse(await run(walz, ['bench', '--json', '--samples', '2', '--iterations', '8', '--warmup-ms', '0'], { cwd: app }));
  assert.equal(report.schema, 'walz-bench-v1');
  assert.equal(report.benchmarks.length, 1);
  assert.equal(report.benchmarks[0].outcome.status, 'measured');
  assert.equal(report.benchmarks[0].outcome.summary.samples_ns.length, 2);
  await writeFile(join(app, 'tests/fail.test.walz'), '(module (@test "failure" 0))');
  assert.match(await run(walz, ['test'], { cwd: app, expected: 1 }), /1 passed; 1 failed; 0 errors/);
  console.log('Installed CLI: standalone bundle/scopes, async/native run/test/bench and failure exit codes passed.');
} finally {
  await rm(directory, { recursive: true, force: true });
}
