import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { execFile as callback } from "node:child_process";
import { instrument, collect } from "./testing/memory.mjs";
const exec = promisify(callback),
  bytes = (s) => new TextEncoder().encode(s);
import { programs } from "./selfhost-gc/corpus.mjs";

for (const backend of ["core", "component"])
  test(
    `${backend}: checked GC types and codegen`,
    { timeout: 180000 },
    async () => {
      const dir = await mkdtemp(join(tmpdir(), "walz-selfhost-gc-"));
      try {
        await (
          await import(backend === "core" ? "./build-core.mjs" : "./build.mjs")
        ).build("tests/component-fixtures/selfhost.walz", dir);
        await instrument(dir, backend);
        const { instantiate } = await import(
          pathToFileURL(join(dir, "index.js"))
        );
        let arena;
        const value = await instantiate({
          instantiateCore: async (m, imports) => {
            const i = await WebAssembly.instantiate(m, imports);
            if (i.exports.live) arena = i.exports;
            return i;
          },
        });
        const clean = () => {
          assert.equal(arena.live(), 0);
          assert.ok(Object.values(value.__memory()).every((n) => n === 0));
        };
        for (const [name, source, expected] of programs) {
          const check = value.api.check(bytes(source));
          assert.equal(check.tag, "ok", name + " " + JSON.stringify(check));
          const result = value.api.compile(bytes(source));
          assert.equal(result.tag, "ok", name + " " + JSON.stringify(result));
          assert.ok(WebAssembly.validate(result.val), name);
          clean();
          const ours = (await WebAssembly.instantiate(result.val)).instance
            .exports;
          if (expected === "trap")
            assert.throws(() => ours.f(), WebAssembly.RuntimeError);
          else assert.equal(ours.f(), expected, name);
          if (!source.includes("(@")) {
            const input = join(dir, "reference.wat"),
              output = join(dir, "reference.wasm");
            await writeFile(input, source);
            await exec("cargo", [
              "run",
              "--locked",
              "--quiet",
              "--example",
              "selfhost-reference",
              "--",
              input,
              output,
            ]);
            const reference = (
              await WebAssembly.instantiate(await readFile(output))
            ).instance.exports;
            if (expected === "trap")
              assert.throws(() => reference.f(), WebAssembly.RuntimeError);
            else assert.equal(reference.f(), ours.f(), name);
            const own = join(dir, "own.wasm"),
              canonOwn = join(dir, "own.canonical.wasm"),
              canonRef = join(dir, "ref.canonical.wasm");
            await writeFile(own, result.val);
            for (const [input, target] of [
              [own, canonOwn],
              [output, canonRef],
            ])
              await exec("cargo", [
                "run",
                "--locked",
                "--quiet",
                "--example",
                "selfhost-reference",
                "--",
                "--gc-canonical",
                input,
                target,
              ]);
            assert.deepEqual(
              await readFile(canonOwn),
              await readFile(canonRef),
              name + " exact GC canonical output",
            );
          }
        }
        for (const source of [
          "(module (type $S (struct (field i32))) (func (struct.set $S 0 (struct.new_default $S) (i32.const 1))))",
          "(module (type $S (struct (field i8))) (func (result i32) (struct.get $S 0 (struct.new_default $S))))",
          "(module (type $A (array i32)) (func (array.set $A (array.new_default $A (i32.const 1)) (i32.const 0) (i32.const 1))))",
          "(module (type $S (struct (field (ref $S)))) (func (result (ref $S)) (struct.new_default $S)))",
          "(module (type $S (struct)) (func (result (ref $S)) (ref.null $S)))",
          "(module (type $A (struct)) (type $B (array i32)) (func (param $b (ref $B)) (result i32) (struct.get $A 0 (local.get $b))))",
          "(module (type $S (struct)) (func (local $s (ref $S)) (nop)))",
          "(module (type $B (struct)) (type $D (sub $B (struct))))",
          "(module (type $B (sub (struct (field (mut i32))))) (type $D (sub $B (struct (field (mut i64))))))",
          "(module (type $A (struct (field (ref null $B)))) (type $B (struct)))",
          "(module (type $S (struct (field (mut i32)))) (func $write (param $s (ref $S)) (struct.set $S 0 (local.get $s) (i32.const 1))) (func (param $s (ref $S)) (@effect pure) (call $write (local.get $s))))",

          "(module (type $S (struct)) (type $S (array i32)))",
          "(module (type $S (struct (field $v i32) (field $v i64))))",
          "(module (type $S (struct)) (func (result (ref null $S)) (select (unreachable) (ref.null $S) (i32.const 0))))",
          "(module (type $S (struct)) (func (result (ref $S)) (unreachable) (i32.const 1)))",
          "(module (func (result i64) (i64.add (unreachable) (ref.as_non_null (unreachable)))))",
          "(module (type $A (array (mut i8))) (type $B (array i16)) (func (param $a (ref $A)) (param $b (ref $B)) (array.copy $A $B (local.get $a) (i32.const 0) (local.get $b) (i32.const 0) (i32.const 1))))",
          "(module (type $S (struct (field (mut i32)))) (func $a (param $s (ref $S)) (@effect pure) (call $b (local.get $s))) (func $b (param $s (ref $S)) (call $c (local.get $s))) (func $c (param $s (ref $S)) (struct.set $S 0 (local.get $s) (i32.const 1))))",
        ]) {
          assert.equal(value.api.check(bytes(source)).tag, "err", source);
          assert.equal(value.api.compile(bytes(source)).tag, "err", source);
          clean();
        }

        for (const source of [
          "(module (type $S (struct (field (mut i32)))) (func (result (ref $S)) (@effect pure) (struct.new_default $S)))",
          "(module (type $A (array (mut i32))) (func (result (ref $A)) (@effect pure) (array.new_default $A (i32.const 1))))",
          "(module (type $S (struct (field (mut i32)))) (func (param $s (ref $S)) (result i32) (@effect pure) (struct.get $S 0 (local.get $s))))",
          "(module (type $S (struct (field (mut i32)))) (func (param $s (ref $S)) (@effect pure) (unreachable) (drop (struct.get $S 0 (local.get $s)))))",
        ]) {
          for (const result of [
            value.api.check(bytes(source)),
            value.api.compile(bytes(source)),
          ]) {
            assert.equal(result.tag, "err", source);
            assert.equal(result.val.code, 123, source);
          }
          clean();
        }

        const sample = value.api.compile(
          await readFile("examples/selfhost/gc.walz"),
        );
        assert.equal(sample.tag, "ok");
        assert.equal(
          (await WebAssembly.instantiate(sample.val)).instance.exports.main(),
          42n,
        );
        clean();
        const samplePath = join(dir, "gc-sample.wasm");
        await writeFile(samplePath, sample.val);
        assert.equal(
          (
            await exec("wasmtime", [
              "run",
              "-W",
              "gc=y,function-references=y",
              "--invoke",
              "main",
              samplePath,
            ])
          ).stdout.trim(),
          "42",
        );
        const field = (storage, mutable = 0) => ({ storage, mutable });
        const type = (kind, fields, extra = {}) => ({
          kind,
          fields,
          parent: -1,
          finality: 1,
          start: 0,
          end: 1,
          explicit: 0,
          ...extra,
        });
        const ins = (op, imm = 0) => ({
          op,
          immediate: BigInt(imm),
          start: 0,
          end: 0,
        });
        const raw = [];
        function add(types, code, result = 127, params = [], locals = []) {
          raw.push({ types, params, locals, result, code });
        }
        add([type(0, [field(127)])], [ins(65, 42), ins(256), ins(258)]);
        add([type(0, [field(120)])], [ins(65, 255), ins(256), ins(259)]);
        add([type(0, [field(120)])], [ins(65, 255), ins(256), ins(258)]);
        add([type(0, [field(127)])], [ins(257), ins(65, 1), ins(261)], 64);
        add([type(0, [field(127, 1)])], [ins(257), ins(65, 1), ins(261)], 64);
        add([type(0, [])], [ins(208, 0)], 512);
        add([type(0, [])], [ins(208, 0)], 513);
        add([type(0, [field(512)])], [ins(257)], 512);
        add([type(0, [field(513)])], [ins(257)], 512);
        add([type(0, [])], [ins(0), ins(208, 0), ins(65, 1), ins(27)], 513);
        add([], [ins(0), ins(212)], 127);
        add([], [ins(0), ins(212)], 466);
        add([type(0, [])], [], 64, [], [512]);
        add([type(0, [], { parent: 0 })], [], 64);
        add([type(0, [field(515)])], [], 64);
        add([type(0, [], { end: 2 })], [], 64);
        add([type(0, [], { explicit: 2 })], [], 64);
        add([type(1, [field(127), field(126)])], [], 64);
        add([type(0, [field(127, 2)])], [], 64);
        add([type(0, [])], [ins(256, 1n << 32n)], 512);
        add([type(0, [])], [ins(208, 42)], 513);
        const lattice = [-18, -19, -20, -21, -22, -23, 0, 1].flatMap((h) => [
          512 + 2 * h,
          513 + 2 * h,
        ]);
        const latticeTypes = [
          type(0, [field(127)]),
          type(1, [field(127)], { start: 1, end: 2 }),
        ];
        for (const a of lattice)
          for (const b of lattice) add(latticeTypes, [ins(32, 0)], b, [a]);
        let seed = 0x6c50a19;
        const random = () => {
          seed ^= seed << 13;
          seed ^= seed >>> 17;
          seed ^= seed << 5;
          return seed >>> 0;
        };
        for (let i = 0; i < 64; i++) {
          const storage = [127, 126, 119, 120][i % 4],
            constant = storage === 126 ? 66 : 65;
          const get =
            storage === 119 || storage === 120
              ? random() & 1
                ? 259
                : 260
              : 258;
          const code = [ins(constant, random() | 0), ins(256), ins(get)];
          const types = [type(0, [field(storage, i & 1)])];
          add(types, code, storage === 126 ? 126 : 127);
          add(
            types,
            code.filter((_, j) => j !== random() % code.length),
            storage === 126 ? 126 : 127,
          );
        }
        const input = join(dir, "gc-ir.json"),
          output = join(dir, "gc-ir-results.json");
        await writeFile(
          input,
          JSON.stringify(raw, (_, v) =>
            typeof v === "bigint" ? String(v) : v,
          ),
        );
        await exec("cargo", [
          "run",
          "--locked",
          "--quiet",
          "--example",
          "selfhost-reference",
          "--",
          "--gc-ir",
          input,
          output,
        ]);
        const reference = JSON.parse(await readFile(output));
        assert.ok(reference.filter(Boolean).length >= 64);
        for (const [i, c] of raw.entries()) {
          const r = value.api.checkGcIr(
            c.types,
            c.params,
            c.locals,
            c.result,
            c.code,
          );
          assert.equal(
            r.tag === "ok",
            reference[i],
            `raw GC IR ${i}: ${JSON.stringify(c, (_, v) => (typeof v === "bigint" ? String(v) : v))}`,
          );
          clean();
        }
        await collect();
        clean();
      } finally {
        await rm(dir, { recursive: true, force: true });
      }
    },
  );
