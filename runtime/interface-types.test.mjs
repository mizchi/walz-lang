import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

for (const backend of ['core', 'component']) {
  test(`${backend}: nominal portable types preserve records, enum names, flags and unsigned limits`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'wlt-interface-types-'));
    try {
      const { build } = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      const names=Array.from({length:32},(_,i)=>`f${i}`);
      const source=(await readFile('tests/component-fixtures/interface-types.wlt','utf8')).trimEnd();
      const input=join(directory,'input.wlt');
      await writeFile(input,source.slice(0,-1)+`
        (type $Flags32 (%newtype i32))
        (%abi.type $Flags32 (flags ${names.map(n=>JSON.stringify(n)).join(' ')}))
        (func $flags32 (export "flags32") (param $v $Flags32) (result $Flags32) (%effect pure) $v))`);
      await build(input, directory);
      const { instantiate } = await import(pathToFileURL(join(directory, 'index.js')));
      let allocator;
      const { api } = await instantiate({ instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) allocator = instance.exports;
        return instance;
      }});
      const item = { name: '日本語\u0000text', count: 4294967295, color: 'blue' };
      for (let i = 0; i < 1000; i++) assert.deepEqual(api.item(item), item);
      assert.deepEqual(api.items([item, { ...item, name: '', color: 'red', count: 0 }]), [item, { ...item, name: '', color: 'red', count: 0 }]);
      assert.equal(api.count(4294967295), 4294967295);
      assert.equal(api.wide(18446744073709551615n), 18446744073709551615n);
      assert.deepEqual(api.access({ read: true, write: false }), { read: true, write: false });
      const flags=Object.fromEntries(names.map((n,i)=>[n,[0,15,16,31].includes(i)]));
      assert.deepEqual(api.flags32(flags),flags,'flags preserve the highest canonical bit');
      assert.throws(() => api.item({ ...item, color: 'unknown' }));
      if (backend === 'core') {
        assert.throws(() => api.count(-1), /u32/);
        assert.throws(() => api.count(4294967296), /u32/);
        assert.throws(() => api.wide(-1n), /u64/);
      }
      assert.ok(allocator);
      assert.equal(allocator.live(), 0);
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
}
