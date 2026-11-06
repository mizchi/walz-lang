// Native Rust profiling on macOS. Timing comparisons use separate Criterion runs.
import { execFile as rawExec, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, open, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { createHash } from 'node:crypto';

const exec = promisify(rawExec);
const [id, filename, secondsText = '5'] = process.argv.slice(2);
const seconds = Number(secondsText);
if (!/^(check|compile|check-error)\/[a-z0-9-]+$/.test(id ?? '') || !filename || !Number.isInteger(seconds) || seconds < 1 || seconds > 55) {
  throw new Error('usage: frontend-profile.mjs check|compile/CASE output.txt [seconds: 1..55]');
}
if (process.platform !== 'darwin') throw new Error('this helper uses macOS sample; use Criterion --profile-time with perf on Linux');
const built = await exec('cargo', ['bench', '--locked', '--bench', 'frontend', '--no-run', '--message-format=json'], { maxBuffer: 8 * 1024 * 1024 });
const artifact = built.stdout.split('\n').filter(line => line.startsWith('{')).map(JSON.parse)
  .find(item => item.reason === 'compiler-artifact' && item.target.name === 'frontend' && item.executable);
if (!artifact) throw new Error('benchmark executable was not produced');
const output = resolve(filename);
await mkdir(dirname(output), { recursive: true });
await (await open(output, 'wx')).close();
const log = await open(output + '.log', 'wx');
const child = spawn(artifact.executable, ['--bench', `^${id}$`, '--profile-time', String(seconds + 2)], { stdio: ['ignore', log.fd, log.fd] });
const exited = new Promise((ok, fail) => { child.once('error', fail); child.once('close', code => ok(code)); });
try {
  await new Promise(ok => setTimeout(ok, 300));
  await exec('/usr/bin/sample', [String(child.pid), String(seconds), '1', '-file', output]);
  if (await exited !== 0) throw new Error('Criterion profiling loop failed');
  await writeFile(output + '.json', JSON.stringify({ benchmark: id, seconds, interval_ms: 1,
    executable: artifact.executable, executable_sha256: createHash('sha256').update(await readFile(artifact.executable)).digest('hex') }, null, 2) + '\n', { flag: 'wx' });
  console.log(output);
} finally {
  child.kill('SIGKILL');
  await exited;
  await log.close();
}
