import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument} from './testing/memory.mjs';

for (const backend of ['core', 'component']) {
  test(`${backend}: inferred hygienic macros preserve ABI values and release copies`, {timeout:60000}, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'walz-user-macros-'));
    try {
      const {build} = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/user-macros.walz', directory);
      await instrument(directory, backend);
      const {instantiate} = await import(pathToFileURL(join(directory, 'index.js')));
      let memory;
      const {api, __memory} = await instantiate({instantiateCore: async (module,imports) => {
        const instance = await WebAssembly.instantiate(module,imports);
        if (instance.exports.live) memory = instance.exports;
        return instance;
      }});
      for (const text of ['', '日本語 🦀', 'x'.repeat(65536)]) {
        assert.equal(await api.main(),42);
        for (const n of [0n,21n,-1n,9223372036854775807n])
          assert.equal(await api.wide(n),BigInt.asIntN(64,n*2n));
        assert.equal(await api.text(text),text);
        assert.equal(await api.composed(20n,22n),42n);
        assert.equal(await api.composed(9223372036854775807n,1n),-9223372036854775808n);
        assert.equal(await api.unwrap(undefined),0n);
        for (const n of [0n,42n,-1n,9223372036854775807n]) {
          assert.equal(await api.unwrap(n),n);
          assert.equal(await api.first(n),n);
        }
        assert.equal(memory.live(),0);
        assert.ok(Object.values(__memory()).every(n=>n===0));
      }
    } finally { await rm(directory,{recursive:true,force:true}); }
  });
}
