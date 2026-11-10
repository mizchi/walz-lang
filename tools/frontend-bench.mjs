// Protect named Criterion baselines from accidental replacement.
import { execFileSync, spawnSync } from 'node:child_process';
import { access, readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';

const [mode, label, filter = ''] = process.argv.slice(2);
if (!['save', 'compare'].includes(mode) || !/^[a-zA-Z0-9_-]+$/.test(label ?? '')) {
  throw new Error('usage: frontend-bench.mjs save|compare LABEL [filter]');
}
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], { encoding: 'utf8' }));
const base = resolve(process.env.CRITERION_HOME ?? `${metadata.target_directory}/criterion`);
const directory = join(base, 'frontend-v7');
let entries;
try { entries = await readdir(directory, { recursive: true, withFileTypes: true }); }
catch (error) { if (error.code !== 'ENOENT') throw error; entries = []; }
const candidates = entries.filter(entry => entry.isDirectory() && entry.name === label);
const exists = (await Promise.all(candidates.map(async entry => {
  try { await access(join(entry.parentPath, entry.name, 'sample.json')); return true; }
  catch (error) { if (error.code !== 'ENOENT') throw error; return false; }
}))).some(Boolean);
if (mode === 'save' && exists) throw new Error(`baseline ${label} already exists; choose a new label`);
if (mode === 'compare' && !exists) throw new Error(`baseline ${label} does not exist`);
const child = spawnSync('cargo', ['bench', '--locked', '--bench', 'frontend', '--', filter, mode === 'save' ? '--save-baseline' : '--baseline', label], { stdio: 'inherit', env: { ...process.env, CRITERION_HOME: base } });
if (child.error) throw child.error;
process.exitCode = child.status ?? 1;
