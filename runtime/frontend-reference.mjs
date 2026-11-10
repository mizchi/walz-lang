// Compatibility oracle for frontend tuning. Capture once, never overwrite it.
import { execFile as rawExec } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const exec = promisify(rawExec);
const root = fileURLToPath(new URL('..', import.meta.url)).replace(/\/$/, '');
const digest = text => createHash('sha256').update(text).digest('hex');

export async function reference(cli = join(root, 'target/debug/walz')) {
  const directory = join(root, 'benches/fixtures/frontend-v6');
  const manifest = JSON.parse(await readFile(join(directory, 'manifest.json')));
  const cases = {};
  for (const name of Object.keys(manifest.files)) {
    if (name === 'type-error.walz') continue;
    const source = join(directory, name);
    const hashes = {};
    for (const command of ['ir', 'analyze', 'wat']) {
      const result = await exec(cli, [command, "--legacy-syntax", source], { cwd: root, maxBuffer: 32 * 1024 * 1024 });
      hashes[command] = digest(result.stdout);
    }
    cases[name] = hashes;
  }
  let diagnostic;
  try {
    await exec(cli, ['check', '--legacy-syntax', '--json', join(directory, 'type-error.walz')], { cwd: root });
    throw new Error('negative fixture was accepted');
  } catch (error) {
    if (error.code !== 1) throw error;
    diagnostic = JSON.parse(error.stdout.replaceAll(root, '<repo>'));
  }
  return { schema: 'walz-frontend-reference-v6', inputs: manifest, cases, diagnostic };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [cli, output] = process.argv.slice(2);
  if (!cli || !output) throw new Error('usage: frontend-reference.mjs CLI output.json');
  await writeFile(output, JSON.stringify(await reference(cli), null, 2) + '\n', { flag: 'wx' });
}
