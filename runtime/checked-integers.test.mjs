import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component", "native-utf8", "native-js-strings"])
  test(`${backend}: checked integers agree with unbounded BigInt arithmetic`, { timeout: 120000 }, async () => {
    const root = await mkdtemp(join(tmpdir(), "walz-checked-integers-"));
    try {
      const directory = join(root, "integers");
      let build;
      if (backend.startsWith("native-")) {
        const { build: native } = await import("./build-js-native.mjs");
        build = (source, output) => native(source, output, { mode: backend.slice(7) });
      } else {
        ({ build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs"));
      }
      await build("tests/component-fixtures/checked-integers.walz", directory);
      if (!backend.startsWith("native-")) await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      let arena;
      const value = await instantiate({ instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      const { api } = value;
      const clean = () => {
        if (arena) assert.equal(arena.live(), 0);
        if (backend.startsWith("native-")) {
          assert.equal(value.__memory().hostHandles, 0);
          assert.equal(value.__memory().boundaryBuffers, 0);
        } else assert.ok(Object.values(value.__memory()).every(n => n === 0));
      };
      assert.equal(api.order(), 123456);
      for (const width of [32, 64]) {
        const bits = BigInt(width), maximum = (1n << bits) - 1n, high = 1n << (bits - 1n);
        const scalar = n => width === 32 ? Number(BigInt.asIntN(width, n)) : BigInt.asIntN(width, n);
        const check = (operation, a, b) => {
          let result;
          if (operation === "add") result = a + b;
          else if (operation === "mul") result = a * b;
          else if (b !== 0n && (b & (b - 1n)) === 0n) result = ((a + b - 1n) / b) * b;
          const expected = result !== undefined && result <= maximum ? scalar(result) : undefined;
          assert.equal(api[operation + width](scalar(a), scalar(b)), expected, `${operation}${width}(${a},${b})`);
          clean();
        };
        const square = 1n << (bits / 2n);
        const values = [0n, 1n, 2n, 3n, 7n, 8n, 9n, square - 1n, square, square + 1n,
          high - 1n, high, high + 1n, maximum - 1n, maximum];
        const alignments = [0n, 3n, 5n, 6n, maximum - 1n, maximum,
          ...Array.from({ length: width }, (_, n) => 1n << BigInt(n))];
        for (const a of values) {
          for (const b of values) { check("add", a, b); check("mul", a, b); }
          for (const alignment of alignments) check("align", a, alignment);
        }
        let seed = 173;
        const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
        const random = () => width === 32 ? BigInt(next()) : (BigInt(next()) << 32n) | BigInt(next());
        for (let round = 0; round < 200; round++) {
          const a = random(), b = random();
          check("add", a, b); check("mul", a, b);
          if (a !== 0n) {
            const quotient = maximum / a;
            check("mul", a, quotient);
            if (quotient < maximum) check("mul", a, quotient + 1n);
          }
          const available = maximum - a;
          check("add", a, available);
          if (available < maximum) check("add", a, available + 1n);
          check("align", a, 1n << BigInt(next() % width));
          check("align", a, b);
        }
      }
      await build("examples/checked-layout.walz", join(root, "layout"));
      const { instantiate: layout } = await import(pathToFileURL(join(root, "layout/index.js")));
      const { api: l } = await layout();
      assert.equal(l.main(), 42);
      for (const [args, expected] of [
        [[10, 4, 1, 2, 1024], 42], [[0, 4, 0, 8, 1024], 0],
        [[-1, 2, 0, 1, -1], undefined], [[-1, 1, 1, 1, -1], undefined],
        [[1, 1, 0, 3, 1024], undefined], [[3, 1, 0, 8, 7], undefined],
      ]) assert.equal(l.layout(...args), expected);
    } finally { await rm(root, { recursive: true, force: true }); }
  });
