import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { execFile as callback } from "node:child_process";
import { pathToFileURL } from "node:url";
import { instrument, collect } from "./testing/memory.mjs";
import {
  programs,
  invalid,
  projects,
  badProjects,
} from "./selfhost-front/corpus.mjs";
import { renderBootstrapCore } from "../tools/selfhost/embed-core.mjs";
test("bootstrap core resource matches its maintained source", async () => {
  assert.equal(
    await readFile("walz/frontend/core-source.walz", "utf8"),
    renderBootstrapCore(await readFile("walz/frontend/bootstrap-core.sexp")),
  );
});
const exec = promisify(callback);
const bytes = (s) => new TextEncoder().encode(s);
const files = (p) =>
  p.map(([path, source]) => ({ path, source: bytes(source) }));
for (const backend of ["core", "component"])
  test(
    `${backend}: bootstrap modules, lowering, effects, core and monomorphization`,
    { timeout: 240000 },
    async () => {
      const dir = await mkdtemp(join(tmpdir(), "walz-selfhost-front-"));
      try {
        await (
          await import(backend === "core" ? "./build-core.mjs" : "./build.mjs")
        ).build("examples/selfhost/portable.walz", dir);
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
        const { api } = value;
        const safe = (name, f, ...args) => {
          try {
            return f(...args);
          } catch (e) {
            assert.fail(name + ": " + e.stack);
          }
        };
        const clean = () => {
          assert.equal(arena.live(), 0);
          assert.ok(Object.values(value.__memory()).every((n) => n === 0));
        };
        for (const [name, source, expected] of programs) {
          const checked = safe(name, api.check, bytes(source));
          assert.equal(checked.tag, "ok", name + " " + JSON.stringify(checked));
          clean();
          const r = safe(name, api.compile, bytes(source));
          assert.equal(r.tag, "ok", name + " " + JSON.stringify(r));
          assert.ok(WebAssembly.validate(r.val));
          clean();
          const { instance } = await WebAssembly.instantiate(r.val);
          if (expected === "trap")
            assert.throws(() => instance.exports.f(), WebAssembly.RuntimeError);
          else assert.equal(instance.exports.f(), expected, name);
        }
        for (const [name, source, code] of invalid)
          for (const r of [
            safe(name, api.check, bytes(source)),
            safe(name, api.compile, bytes(source)),
          ]) {
            assert.equal(r.tag, "err", name + " " + JSON.stringify(r));
            assert.equal(r.val.code, code, name);
            clean();
          }
        for (const [name, p, expected] of projects) {
          const checked = api.checkProject(files(p));
          assert.equal(checked.tag, "ok", name + " " + JSON.stringify(checked));
          const r = safe(name, api.compileProject, files(p));
          assert.equal(r.tag, "ok", name + " " + JSON.stringify(r));
          assert.ok(WebAssembly.validate(r.val));
          assert.equal(
            (await WebAssembly.instantiate(r.val)).instance.exports.f(),
            expected,
            name,
          );
          clean();
        }
        for (const [name, p, code] of badProjects) {
          const r = safe(name, api.compileProject, files(p));
          assert.equal(r.tag, "err", name + " " + JSON.stringify(r));
          assert.equal(r.val.code, code, name);
          if (name === "source-range") {
            assert.equal(r.val.file, 1);
            assert.equal(
              p[1][1].slice(r.val.start, r.val.end),
              "(func $bad (result i64) (i32.const 42))",
            );
          }
          clean();
        }
        const queryOnly =
          '(module (@macro $m (rule (?x:expr) (infer (?T ?x)) (i32.const 42))) (func (export "f") (result i32) (@$m (i32.const 1))))';
        assert.deepEqual(
          api.check(bytes(queryOnly)),
          { tag: "ok", val: 1 },
          "inference probes must not be emitted",
        );
        const dedup =
          '(module (func $id (@type.params $T) (param $x $T) (result $T) $x) (func (export "f") (result i32) (+ (call $id<i32> 20) (call $id<i32> 22))))';
        assert.deepEqual(api.check(bytes(dedup)), { tag: "ok", val: 2 });
        for (let i = 0; i < 16; i++) {
          assert.equal(api.compile(bytes(programs[0][1])).tag, "ok");
          clean();
        }
        const output = join(dir, "project.wasm");
        await exec("node", [
          "runtime/selfhost.mjs",
          "--check",
          "examples/selfhost/project/main.walz",
          dir,
        ]);
        await exec("node", [
          "runtime/selfhost.mjs",
          "examples/selfhost/project/main.walz",
          output,
          dir,
        ]);
        const wasm = await readFile(output);
        assert.ok(WebAssembly.validate(wasm));
        assert.equal(
          (await WebAssembly.instantiate(wasm)).instance.exports.main(),
          42,
        );
        const native = await exec("wasmtime", [
          "run",
          "-W",
          "gc=y,function-references=y",
          "--invoke",
          "main",
          output,
        ]);
        assert.equal(native.stdout.trim(), "42");
        if (backend === "core")
          assert.equal(
            (
              await exec("target/debug/walz", [
                "run",
                "examples/selfhost/project/main.walz",
              ])
            ).stdout.trim(),
            "42",
          );
        await collect();
        clean();
      } finally {
        await rm(dir, { recursive: true, force: true });
      }
    },
  );
