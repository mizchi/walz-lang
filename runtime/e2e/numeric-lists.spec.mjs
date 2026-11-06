import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: numeric list bulk copies preserve snapshots across memory growth`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async backend => {
      const { instantiate } = await import(new URL("numeric-lists/index.js", location.href).href);
      let arena;
      const { api } = await instantiate({ instantiateCore: async (m, i) => {
        const instance = await WebAssembly.instantiate(m, i);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      const input = Int32Array.from({ length: 132 }, (_, i) => i - 66).subarray(2, 130);
      const get = DataView.prototype.getInt32, set = DataView.prototype.setInt32, counts = [0, 0];
      let copied;
      try {
        DataView.prototype.getInt32 = function (...args) { counts[0]++; return get.apply(this, args); };
        DataView.prototype.setInt32 = function (...args) { counts[1]++; return set.apply(this, args); };
        copied = api.ints(input);
      } finally {
        DataView.prototype.getInt32 = get; DataView.prototype.setInt32 = set;
      }
      const same = Array.from(copied).every((n, i) => n === i - 64);
      input.fill(0);
      const before = arena.memory.buffer.byteLength;
      api.ints(new Int32Array(100000));
      const snapshot = copied[0] === -64 && copied[127] === 63 && copied.buffer !== arena.memory.buffer;
      const doubles = api.doubles(new Float64Array([Math.PI, -0, Infinity, NaN]));
      const floats = api.floats(new Float32Array([Math.PI, -0]));
      const longs = Array.from(api.longs(new BigInt64Array([-9223372036854775808n, 42n, 9223372036854775807n])), String);
      const future = Array.from(await api.later(Promise.resolve(new Int32Array([40, 2]))));
      const stream = await api.first([new Float64Array([Math.PI, -0])]);
      let invalid = false;
      try { api.longs([1n, 2]); } catch { invalid = true; }
      const validAgain = api.ints([42])[0] === 42;
      return {
        same, snapshot, grown: arena.memory.buffer.byteLength > before, longs, future,
        float: floats[0] === Math.fround(Math.PI) && Object.is(floats[1], -0),
        double: doubles[0] === Math.PI && Object.is(doubles[1], -0) && doubles[2] === Infinity && Number.isNaN(doubles[3]),
        stream: stream[0] === Math.PI && Object.is(stream[1], -0),
        bulk: backend !== "core" || counts.every(n => n === 0),
        invalid, validAgain, live: arena.live(),
      };
    }, backend);
    expect(result).toEqual({
      same: true, snapshot: true, grown: true, float: true, double: true, stream: true,
      longs: ["-9223372036854775808", "42", "9223372036854775807"], future: [40, 2],
      bulk: true, invalid: true, validAgain: true, live: 0,
    });
  });
