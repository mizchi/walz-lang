import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { reference } from './frontend-reference.mjs';
import { execFile as rawExec } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

// V8 records the intentional range/value lowering and core byte API additions.
// Earlier references remain immutable; every exception names both exact hashes.
const transition = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v8-transition.json', import.meta.url)));
function historicalWat(version, name, expected) {
  const change = transition.historicalWat[version]?.[name];
  if (!change) return expected;
  assert.equal(change.before, expected, `${version}/${name}: historical hash`);
  assert.notEqual(change.before, change.after);
  return change.after;
}

test('v1 WAT matches the recorded namespace and v8 lowering transitions', async () => {
  const before = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v1.json', import.meta.url)));
  const exec = promisify(rawExec);
  const root = fileURLToPath(new URL('..', import.meta.url));
  for (const [name, expected] of Object.entries(before.cases)) {
    const input = fileURLToPath(new URL(`../benches/fixtures/frontend-v7/${name.replace('.wlt', '.walz')}`, import.meta.url));
    const { stdout } = await exec(root + 'target/debug/walz', ['wat', '--legacy-syntax', input], { maxBuffer: 32 * 1024 * 1024 });
    const normalized = stdout.replaceAll('walz', 'wlt').replaceAll('$wlt-core', '$wlt-std');
    assert.equal(createHash('sha256').update(normalized).digest('hex'), historicalWat(1, name, expected.wat), name);
  }
});

test('v4 WAT matches the recorded namespace and v8 lowering transitions', async () => {
  const before = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v4.json', import.meta.url)));
  const exec = promisify(rawExec);
  const root = fileURLToPath(new URL('..', import.meta.url));
  for (const [name, expected] of Object.entries(before.cases)) {
    const input = fileURLToPath(new URL(`../benches/fixtures/frontend-v7/${name.replace('.wlt', '.walz')}`, import.meta.url));
    const { stdout } = await exec(root + 'target/debug/walz', ['wat', '--legacy-syntax', input], { maxBuffer: 32 * 1024 * 1024 });
    const normalized = stdout.replaceAll('walz', 'wlt');
    assert.equal(createHash('sha256').update(normalized).digest('hex'), historicalWat(4, name, expected.wat), name);
  }
});

test('v5 WAT matches exactly with the recorded v8 lowering transition', async () => {
  const before = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v5.json', import.meta.url)));
  const exec = promisify(rawExec);
  const root = fileURLToPath(new URL('..', import.meta.url));
  for (const [name, expected] of Object.entries(before.cases)) {
    const input = fileURLToPath(new URL(`../benches/fixtures/frontend-v7/${name}`, import.meta.url));
    const { stdout } = await exec(root + 'target/debug/walz', ['wat', '--legacy-syntax', input], { maxBuffer: 32 * 1024 * 1024 });
    assert.equal(createHash('sha256').update(stdout).digest('hex'), historicalWat(5, name, expected.wat), name);
  }
});

test('v6 WAT matches exactly with the recorded v8 lowering transition', async () => {
  const before = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v6.json', import.meta.url)));
  const exec = promisify(rawExec);
  const root = fileURLToPath(new URL('..', import.meta.url));
  for (const [name, expected] of Object.entries(before.cases)) {
    const input = fileURLToPath(new URL(`../benches/fixtures/frontend-v7/${name}`, import.meta.url));
    const { stdout } = await exec(root + 'target/debug/walz', ['wat', '--legacy-syntax', input], { maxBuffer: 32 * 1024 * 1024 });
    assert.equal(createHash('sha256').update(stdout).digest('hex'), historicalWat(6, name, expected.wat), name);
  }
});

test('v7 inputs and outputs match except for the exact recorded v8 transitions', async () => {
  const expected = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v7.json', import.meta.url)));
  for (const [name, hash] of Object.entries(expected.inputs.files)) {
    const data = await readFile(new URL(`../benches/fixtures/frontend-v7/${name}`, import.meta.url));
    assert.equal(createHash('sha256').update(data).digest('hex'), hash, name);
  }
  for (const [name, fields] of Object.entries(transition.cases)) {
    for (const [field, change] of Object.entries(fields)) {
      assert.equal(expected.cases[name][field], change.before, `${name}/${field}: historical hash`);
      assert.notEqual(change.before, change.after);
      expected.cases[name][field] = change.after;
    }
  }
  assert.deepEqual(await reference(undefined, 7), expected);
});

test('frontend tuning preserves v8 inputs, typed IR, analysis, WAT and diagnostics', async () => {
  const expected = JSON.parse(await readFile(new URL('../tests/fixtures/frontend-reference-v8.json', import.meta.url)));
  for (const [name, hash] of Object.entries(expected.inputs.files)) {
    const data = await readFile(new URL(`../benches/fixtures/frontend-v8/${name}`, import.meta.url));
    assert.equal(createHash('sha256').update(data).digest('hex'), hash, name);
  }
  assert.deepEqual(await reference(), expected);
});
