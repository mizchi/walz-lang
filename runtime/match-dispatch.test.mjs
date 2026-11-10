import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument} from './testing/memory.mjs';

for (const backend of ['core', 'component']) {
  test(`${backend}: dense match dispatch preserves values and evaluates its subject once`, {timeout:60000}, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'wlt-match-dispatch-'));
    try {
      const {build} = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/match-dispatch.wlt', directory);
      await instrument(directory, backend);
      const {instantiate} = await import(pathToFileURL(join(directory, 'index.js')));
      const {api, __memory} = await instantiate();
      for (let op = 0; op < 8; op++) {
        for (const n of [-2147483648, -1, 0, 42, 2147483647]) {
          assert.equal(await api.run(op, n), (n + op) | 0);
          assert.equal(await api.main(), 42);
          assert.ok(Object.values(__memory()).every(n => n === 0));
        }
      }
    } finally {
      await rm(directory, {recursive:true, force:true});
    }
  });
}
