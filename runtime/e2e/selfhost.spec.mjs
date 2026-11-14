import { test, expect } from "@playwright/test";
import {
  programs as frontPrograms,
  invalid as frontInvalid,
  projects as frontProjects,
  badProjects as frontBadProjects,
} from "../selfhost-front/corpus.mjs";
for (const backend of ["core", "component"])
  test(`${backend}: Walz compiler emits and executes Wasm in Chromium`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("selfhost/index.js", location.href)
      );
      let arena;
      const { api } = await instantiate({
        instantiateCore: async (m, imports) => {
          const instance = await WebAssembly.instantiate(m, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        },
      });
      const encode = (s) => new TextEncoder().encode(s);
      const source =
        '(module (func $plus (param $x i32) (result i32) (+ $x 2)) (func (export "答え🦀") (result i32) (call $plus 40)))';
      const bad = api.compile(encode("(module (func (result i32) $missing))"));
      const parsed = api.inspect(encode("(; outer (; inner ;) ;) (module)"));
      const output = api.compile(encode(source));
      const binary = await WebAssembly.instantiate(output.val);
      const wideSource =
        '(module (func (export "sum") (param $n i64) (result i64) (local $acc i64) (block $done (loop $again (br_if $done (i64.eqz $n)) (local.set $acc (+ $acc $n)) (local.set $n (- $n 1)) (br $again))) $acc))';
      const checked = api.check(encode(wideSource));
      const wide = await WebAssembly.instantiate(
        api.compile(encode(wideSource)).val,
      );
      const mismatch = api.check(
        encode("(module (func (result i64) (i32.const 1)))"),
      );
      return {
        status: output.tag,
        answer: binary.instance.exports["答え🦀"](),
        valid: WebAssembly.validate(output.val),
        error: bad.tag,
        code: bad.val.code,
        roots: parsed.val.roots.length,
        live: arena.live(),
        checked: checked.tag,
        functions: checked.val,
        sum: String(wide.instance.exports.sum(100n)),
        mismatch: mismatch.val.code,
      };
    });
    expect(result).toEqual({
      status: "ok",
      answer: 42,
      valid: true,
      error: "err",
      code: 109,
      roots: 1,
      live: 0,
      checked: "ok",
      functions: 1,
      sum: "5050",
      mismatch: 110,
    });
  });

for (const backend of ["core", "component"])
  test(`${backend}: GC compiler output and diagnostics work in Chromium`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("selfhost/index.js", location.href)
      );
      const { programs } = await import("/gc-corpus.mjs");
      let arena;
      const { api } = await instantiate({
        instantiateCore: async (m, imports) => {
          const i = await WebAssembly.instantiate(m, imports);
          if (i.exports.live) arena = i.exports;
          return i;
        },
      });
      const encode = (s) => new TextEncoder().encode(s);
      let count = 0,
        traps = 0;
      for (const [name, source, expected] of programs) {
        const checked = api.check(encode(source));
        if (checked.tag !== "ok")
          throw new Error(name + JSON.stringify(checked));
        const output = api.compile(encode(source));
        if (output.tag !== "ok" || !WebAssembly.validate(output.val))
          throw new Error(name);
        const { instance } = await WebAssembly.instantiate(output.val);
        if (expected === "trap") {
          let trapped = false;
          try {
            instance.exports.f();
          } catch (e) {
            if (!(e instanceof WebAssembly.RuntimeError)) throw e;
            trapped = true;
          }
          if (!trapped) throw new Error(name + " must trap");
          traps++;
        } else if (instance.exports.f() !== expected)
          throw new Error(name + " output");
        if (arena.live() !== 0) throw new Error("ABI allocation retained");
        count++;
      }
      const sample = api.compile(
        encode(await (await fetch("/gc-example.walz")).text()),
      );
      const { instance } = await WebAssembly.instantiate(sample.val);
      const bad = api.check(
        encode(
          "(module (func (param $x (ref null any)) (result (ref null eq)) (local.get $x)))",
        ),
      );
      return {
        count,
        traps,
        sum: String(instance.exports.main()),
        bad: bad.tag,
        live: arena.live(),
      };
    });
    expect(result).toEqual({
      count: 22,
      traps: 3,
      sum: "42",
      bad: "err",
      live: 0,
    });
  });

for (const backend of ["core", "component"])
  test(`${backend}: modular frontend validates and executes in Chromium`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("selfhost/index.js", location.href)
      );
      const { programs, invalid, projects, badProjects } = await import(
        "/front-corpus.mjs"
      );
      let arena;
      const { api } = await instantiate({
        instantiateCore: async (m, imports) => {
          const instance = await WebAssembly.instantiate(m, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        },
      });
      const bytes = (s) => new TextEncoder().encode(s);
      const files = (p) =>
        p.map(([path, source]) => ({ path, source: bytes(source) }));
      const clean = () => {
        if (arena.live() !== 0) throw new Error("ABI allocation retained");
      };
      const run = async (name, r, expected) => {
        if (r.tag !== "ok" || !WebAssembly.validate(r.val))
          throw new Error(name + JSON.stringify(r));
        const { instance } = await WebAssembly.instantiate(r.val);
        if (expected === "trap") {
          let trapped = false;
          try {
            instance.exports.f();
          } catch (e) {
            if (!(e instanceof WebAssembly.RuntimeError)) throw e;
            trapped = true;
          }
          if (!trapped) throw new Error(name + " must trap");
        } else if (instance.exports.f() !== expected)
          throw new Error(name + " output");
        clean();
      };
      for (const [name, source, expected] of programs) {
        const checked = api.check(bytes(source));
        if (checked.tag !== "ok")
          throw new Error(name + JSON.stringify(checked));
        await run(name, api.compile(bytes(source)), expected);
      }
      for (const [name, source, code, focus] of invalid) {
        for (const r of [
          api.check(bytes(source)),
          api.compile(bytes(source)),
        ]) {
          if (r.tag !== "err" || r.val.code !== code)
            throw new Error(name + JSON.stringify(r));
          if (focus && new TextDecoder().decode(bytes(source).slice(r.val.start, r.val.end)) !== focus)
            throw new Error(name + " lost original UTF-8 range");
          clean();
        }
      }
      for (const [name, project, expected] of projects) {
        const checked = api.checkProject(files(project));
        if (checked.tag !== "ok")
          throw new Error(name + JSON.stringify(checked));
        await run(name, api.compileProject(files(project)), expected);
      }
      for (const [name, project, code] of badProjects) {
        const r = api.compileProject(files(project));
        if (r.tag !== "err" || r.val.code !== code)
          throw new Error(name + JSON.stringify(r));
        if (
          name === "source-range" &&
          (r.val.file !== 1 ||
            project[1][1].slice(r.val.start, r.val.end) !==
              "(func $bad (result i64) (i32.const 42))")
        )
          throw new Error("lost original source location");
        clean();
      }
      const imports = api.imports(
        bytes('(module (@use "./lib.walz" (func $f)))'),
      );
      return {
        programs: programs.length,
        invalid: invalid.length,
        projects: projects.length,
        badProjects: badProjects.length,
        path: imports.val[0].path,
        live: arena.live(),
      };
    });
    expect(result).toEqual({
      programs: frontPrograms.length,
      invalid: frontInvalid.length,
      projects: frontProjects.length,
      badProjects: frontBadProjects.length,
      path: "./lib.walz",
      live: 0,
    });
  });

for (const backend of ["core", "component"])
  test(`${backend}: frontend profiling reuses checked signatures in Chromium`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("selfhost/index.js", location.href));
      const { repeated } = await import("/budgets.mjs");
      const { fields, types, recursive } = await import("/scaling.mjs");
      let arena;
      const { api } = await instantiate({ instantiateCore: async (m, imports) => {
        const instance = await WebAssembly.instantiate(m, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      }});
      const input = [{path: "main.walz", source: new TextEncoder().encode(repeated(100))}];
      const first = api.profileProject(input), second = api.profileProject(input);
      const bad = api.profileProject([{path: "main.walz", source: new TextEncoder().encode('(module (@use "walz:core" (type $unit)) (func $entry (result i32) (i64.const 1)))')}]);
      const project = (source) => [{path:"main.walz",source:new TextEncoder().encode(source)}];
      const layout = api.profileProject(project(fields(1000)));
      const manyTypes = api.profileProject(project(types(300)));
      const rec = api.profileProject(project(recursive(192)));
      return {first, second, bad, layout, manyTypes, rec, live: arena.live()};
    });
    expect(result.first.result).toEqual({tag: "ok", val: 2});
    expect(result.first.stats.signatureComputations).toBe(2);
    expect(result.first.stats.signatureHits).toBeGreaterThan(0);
    expect(result.first.stats.phase).toBe(7);
    expect(result.first.stats.renderedBytes).toBe(0);
    expect(result.first.stats.syntaxNodes).toBeGreaterThan(0);
    expect(result.first.stats.syntaxStorage).toBeGreaterThan(0);
    expect(result.second).toEqual(result.first);
    expect(result.bad.result.tag).toBe("err");
    expect(result.bad.result.val.code).toBe(110);
    expect(result.bad.stats.phase).toBe(6);
    expect(result.layout.result.tag).toBe("ok");
    expect(result.layout.stats.layoutComputations).toBe(1);
    expect(result.layout.stats.fieldComputations).toBe(1);
    expect(result.layout.stats.typeWork).toBeLessThan(4000);
    expect(result.manyTypes.result.tag).toBe("ok");
    expect(result.manyTypes.stats.heapInstances).toBe(300);
    expect(result.rec.result.tag).toBe("ok");
    expect(result.rec.stats.heapInstances).toBe(192);
    expect(result.live).toBe(0);
  });
