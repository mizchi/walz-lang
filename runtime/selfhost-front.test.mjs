import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile, writeFile } from "node:fs/promises";
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
import { connectionErrors } from "./selfhost-front/connections.mjs";
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
        for (const [name, source, code, focus] of invalid)
          for (const r of [
            safe(name, api.check, bytes(source)),
            safe(name, api.compile, bytes(source)),
          ]) {
            assert.equal(r.tag, "err", name + " " + JSON.stringify(r));
            assert.equal(r.val.code, code, name);
            if (focus) assert.equal(new TextDecoder().decode(bytes(source).slice(r.val.start, r.val.end)), focus, name);
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
        const coreBytes = [
          ["main.walz", '(module (@use "walz:core" (type $bytes)) (@use "./bytes.walz" (func $bytes.concat)) (func $entry (export "f") (result i32) (@effect reads writes allocates) (@bytes.get (call $bytes.concat (@bytes.new 7) (@bytes.new 42)) 1)))'],
          ["bytes.walz", await readFile("walz/core/bytes.walz", "utf8")],
        ];
        const concatenated = api.compileProject(files(coreBytes));
        assert.equal(concatenated.tag, "ok", JSON.stringify(concatenated));
        assert.ok(WebAssembly.validate(concatenated.val));
        assert.equal((await WebAssembly.instantiate(concatenated.val)).instance.exports.f(), 42);
        clean();
        // All entry points agree on phase errors and original UTF-8 ranges.
        for (const [name, source, code, focus] of connectionErrors) {
          const input = bytes(source);
          const outcomes = [api.check(input), api.compile(input),
            api.checkProject([{path:"main.walz",source:input}]),
            api.compileProject([{path:"main.walz",source:input}])];
          if (code !== 222) outcomes.push(api.imports(input));
          for (const result of outcomes) {
            assert.equal(result.tag,"err",name);
            assert.equal(result.val.code,code,name);
            assert.equal(new TextDecoder().decode(input.slice(result.val.start,result.val.end)),focus,name);
            clean();
          }
        }
        const binaryEntry=join(dir,"binary-entry.walz");
        await writeFile(binaryEntry,connectionErrors.find(([name])=>name==='implicit-binary-function')[1]);
        await assert.rejects(exec("node",["runtime/selfhost.mjs","--check",binaryEntry,dir]),
          error=>error.stderr.includes("bootstrap compiler error 220") && !error.stderr.includes("ENOENT"));
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
        const singleInput = join(dir, "single.walz"), singleOutput = join(dir, "single.wasm");
        await writeFile(singleInput, '(module (@use "walz:core" (type $unit)) (func $entry (export "f") (result i32) 42))');
        await exec("node", ["runtime/selfhost.mjs", "--check", singleInput, dir]);
        await exec("node", ["runtime/selfhost.mjs", singleInput, singleOutput, dir]);
        const single = await readFile(singleOutput);
        assert.ok(WebAssembly.validate(single));
        assert.equal((await WebAssembly.instantiate(single)).instance.exports.f(), 42);
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
              await exec("cargo", [
                "run", "--locked", "--quiet", "--",
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
