import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { instrument } from './testing/memory.mjs';

const quote = text => '"' + text.replace(/["\\\x00-\x1f]/g, c => c === '"' || c === '\\' ? '\\' + c : '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0')) + '"';
for (const backend of ['core', 'component']) {
  test(`${backend}: tuned std keeps exact escaping, Unicode and cleanup semantics`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), 'walz-stdlib-tuning-'));
    try {
      const { build } = await import(backend === 'core' ? './build-core.mjs' : './build.mjs');
      await build('tests/component-fixtures/stdlib-tuning.walz', directory);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, 'index.js')));
      let arena;
      const { api, __memory } = await instantiate({ instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      const clean = () => {
        assert.equal(arena.live(), 0);
        assert.ok(Object.values(__memory()).every(n => n === 0));
      };
      for (const text of ['null', 'true', '42', '{"x":1}', '"hello"']) {
        assert.equal(api.quote(text), quote(text)); clean();
        assert.deepEqual(api.normalize(text), { tag: 'ok', val: JSON.stringify(JSON.parse(text)) }); clean();
      }
      assert.equal(api.quote('hello'), '"hello"'); clean();
      assert.deepEqual(api.normalize('hello'), { tag: 'err', val: 0 }); clean();
      const inputs = ['', 'abc/XYZ', '日本語🦀', String.fromCharCode(...Array.from({length: 128}, (_, i) => i)), '\u0080\u07ff\u0800\ud7ff\ue000\uffff\udbff\udfff'];
      for (const length of [1, 7, 8, 9, 15, 16, 31, 32, 63, 64, 1023, 4096]) {
        const text = 'a'.repeat(length) + '日🦀';
        for (const escape of ['"', '\\', '\0', '\n', '\t']) inputs.push(escape + text, text + escape, text + escape + text);
        inputs.push(text);
      }
      let seed = 997;
      const alphabet = ['a', 'x', '日', '🦀', '"', '\\', '\0', '\n', '\t', '/'];
      for (let round = 0; round < 200; round++) {
        let text = '';
        for (let i = 0; i < round; i++) {
          seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
          text += alphabet[seed % alphabet.length];
        }
        inputs.push(text);
      }
      for (const text of inputs) {
        const encoded = api.quote(text);
        assert.equal(encoded, quote(text));
        assert.equal(JSON.parse(encoded), text);
        clean();
      }
      for (const parts of [[], [''], ['日', '🦀'], inputs.slice(0, 128), inputs.slice(-32)]) {
        assert.equal(api.chunks(parts), parts.join('')); clean();
      }
      for (const a of inputs.slice(0, 32)) for (const b of ['', 'suffix', '日本語🦀', '\0']) {
        assert.equal(api.concat(a, b), a + b); clean();
      }
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
}
