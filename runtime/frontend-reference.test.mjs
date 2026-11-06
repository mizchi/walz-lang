import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { reference } from './frontend-reference.mjs';
import { execFile as rawExec } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

test('the core package rename preserves v1 WAT apart from generated namespace names', async () => {
  const before = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v1.json', import.meta.url)));
  const exec = promisify(rawExec);
  const root = fileURLToPath(new URL('..', import.meta.url));
  for (const [name, expected] of Object.entries(before.cases)) {
    const input = fileURLToPath(new URL(`../benches/fixtures/frontend-v4/${name}`, import.meta.url));
    const { stdout } = await exec(root + 'target/debug/wlt', ['wat', '--legacy-syntax', input], { maxBuffer: 32 * 1024 * 1024 });
    const normalized = stdout.replaceAll('$wlt-core', '$wlt-std');
    assert.equal(createHash('sha256').update(normalized).digest('hex'), expected.wat, name);
  }
});

test('frontend tuning preserves frozen inputs, typed IR, analysis, WAT and diagnostics', async () => {
  const expected = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v4.json', import.meta.url)));
  for (const [name, hash] of Object.entries(expected.inputs.files)) {
    const data = await readFile(new URL(`../benches/fixtures/frontend-v4/${name}`, import.meta.url));
    assert.equal(createHash('sha256').update(data).digest('hex'), hash, name);
  }
  assert.deepEqual(await reference(), expected);
});
