import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";
import { repeated, functions } from "./selfhost-front/budgets.mjs";
import {fields, aliases as aliasProgram, programs as scalingPrograms} from "./selfhost-front/scaling.mjs";
import {programs as serializationPrograms} from "./selfhost-front/serialization.mjs";
import { instrument, collect } from "./testing/memory.mjs";
import {describe} from './selfhost/profiling.mjs';

test('profile versions retain their phase meanings',()=>{
  assert.equal(describe({phase:5,limit:0}).countersSchema,'walz-selfhost-stats-v1');
  assert.equal(describe({phase:5,limit:7,layoutRequests:0}).phase,'reparse');
  assert.equal(describe({phase:5,limit:7,layoutRequests:0}).countersSchema,'walz-selfhost-stats-v2');
  assert.equal(describe({phase:5,limit:8,syntaxNodes:1}).phase,'syntax');
  assert.equal(describe({phase:5,limit:8,syntaxNodes:1}).limit,'syntax-nodes');
  assert.equal(describe({phase:5,limit:9,syntaxNodes:1}).limit,'syntax-storage');
  assert.equal(describe({phase:4,limit:0,syntaxNodes:1}).phase,'lower');
  assert.equal(describe({phase:5,limit:0,syntaxNodes:1}).countersSchema,'walz-selfhost-stats-v3');
  assert.equal(describe({phase:5,limit:10,syntaxNodes:1,syntaxAtomHits:1}).countersSchema,'walz-selfhost-stats-v4');
  assert.equal(describe({phase:5,limit:10,syntaxNodes:1,syntaxAtomHits:1}).limit,'syntax-work');
});

const exec = promisify(callback);
const files = (source) => [{ path: "main.walz", source: new TextEncoder().encode(source) }];

for (const backend of ["core", "component"])
  test(`${backend}: frontend profiling is isolated and signatures are reused`, { timeout: 180000 }, async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-selfhost-profile-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("examples/selfhost/profile.walz", dir);
      await instrument(dir, backend);
      let arena;
      const value = await (await import(pathToFileURL(join(dir, "index.js")))).instantiate({
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        },
      });
      const clean = () => {
        assert.equal(arena.live(), 0);
        assert.ok(Object.values(value.__memory()).every((n) => n === 0));
      };
      const profile = (source) => {
        const report = value.api.profileProject(files(source));
        clean();
        assert.deepEqual(report.result, value.api.checkProject(files(source)), "profile keeps ordinary diagnostics");
        clean();
        return report;
      };
      const small = profile(repeated(10)), large = profile(repeated(1000));
      assert.equal(small.result.tag, "ok");
      assert.equal(large.result.tag, "ok");
      assert.equal(large.result.val, 2);
      assert.equal(large.stats.phase, 7);
      assert.equal(large.stats.limit, 0);
      assert.ok(large.stats.signatureHits > small.stats.signatureHits);
      assert.equal(large.stats.signatureComputations, large.stats.functionInstances);
      assert.equal(small.stats.signatureComputations, large.stats.signatureComputations);
      const inferred = profile(repeated(50).replaceAll('(drop (call $id<i32> 42))', '(drop (@let $value (call $id<i32> 42) $value))'));
      assert.ok(inferred.stats.returnHits > 0);
      assert.equal(large.stats.instanceComputations, large.stats.heapInstances + large.stats.functionInstances + large.stats.aliasInstances);
      const work = (s) => s.typeWork + s.expressionWork + s.logicalWork + s.namedFieldWork;
      assert.equal(large.stats.files.reduce((sum, f) => sum + work(f), 0), work(large.stats));
      assert.deepEqual(profile(repeated(10)), small, "no cumulative global counters");
      const aliases = profile(`(module
        (@type.alias $Int i32)
        (func $id (@type.params $T) (param $x $T) (result $T) $x)
        (func $entry (export "f") (result i32)
          (block (result i32) (drop (call $id<$Int> 1)) (call $id<i32> 42))))`);
      assert.equal(aliases.result.tag, "ok");
      assert.equal(aliases.stats.functionInstances, 2, "normalized aliases share a specialization");
      const nominal = profile(`(module
        (type $A (struct (field $value i32))) (type $B (struct (field $value i32)))
        (func $id (@type.params $T) (param $x $T) (result $T) $x)
        (func $entry (export "f") (result i32)
          (block (result i32)
            (drop (call $id<(ref $A)> (struct.new $A 1)))
            (drop (call $id<(ref $B)> (struct.new $B 2))) 42)))`);
      assert.equal(nominal.result.tag, "ok");
      assert.equal(nominal.stats.functionInstances, 3, "nominal layouts retain separate cache keys");
      assert.equal(nominal.stats.signatureComputations, 3);
      const recursive = profile(`(module (@use "walz:core" (type $unit))
        (func $count (param $n i32) (result i32)
          (if (result i32) (= $n 0) (then 42) (else (call $count (- $n 1)))))
        (func $entry (export "f") (result i32) (call $count 8)))`);
      assert.equal(recursive.result.tag, "ok");
      assert.equal(recursive.stats.signatureComputations, 2);
      const badCall = '(module (@use "walz:core" (type $unit)) (func $id (param $x i32) (result i32) $x) (func $entry (result i32) (block (result i32) (drop (call $id 1)) (call $id (i64.const 2)))))';
      const bad = profile(badCall);
      assert.equal(bad.result.tag, "err");
      assert.equal(bad.result.val.code, 110);
      assert.equal(badCall.slice(bad.result.val.start, bad.result.val.end), '(call $id (i64.const 2))');
      for (const source of [repeated(10)]) {
        const result = value.api.compileProject(files(source));
        assert.equal(result.tag, "ok");
        assert.ok(WebAssembly.validate(result.val));
        assert.equal((await WebAssembly.instantiate(result.val)).instance.exports.f(), 42);
        clean();
      }
      const fieldSmall = profile(fields(10)), fieldLarge = profile(fields(1000));
      assert.equal(fieldLarge.stats.layoutComputations, 1);
      assert.equal(fieldLarge.stats.fieldComputations, 1);
      assert.ok(fieldLarge.stats.fieldHits > fieldSmall.stats.fieldHits);
      assert.ok(fieldLarge.stats.typeWork < 4000);
      assert.ok(fieldLarge.stats.syntaxAtomHits > 0);
      assert.equal(fieldLarge.stats.syntaxNodes+fieldLarge.stats.syntaxAtomHits,fieldLarge.stats.syntaxVisits);
      for (const [name, source, expected] of scalingPrograms) {
        const report = profile(source);
        assert.equal(report.result.tag, "ok", name);
        const result = value.api.compileProject(files(source));
        assert.equal(result.tag, "ok", name);
        assert.ok(WebAssembly.validate(result.val), name);
        assert.equal((await WebAssembly.instantiate(result.val)).instance.exports.f(), expected, name);
        clean();
      }
      const invalid = profile('(module (@use "walz:core" (type $unit)) (func $entry (result i32) (i64.const 1)))');
      assert.equal(invalid.result.tag, "err");
      assert.equal(invalid.result.val.code, 110);
      assert.equal(invalid.stats.phase, 6);
      const limited = profile(functions(1100));
      assert.equal(limited.result.tag, "err");
      assert.equal(limited.result.val.code, 150);
      assert.equal(limited.stats.phase, 6);
      assert.equal(limited.stats.limit, 0);
      const cacheLimit = profile(aliasProgram(3073));
      assert.equal(cacheLimit.result.tag, "err");
      assert.equal(cacheLimit.result.val.code, 150);
      assert.equal(cacheLimit.stats.phase, 2);
      assert.equal(cacheLimit.stats.limit, 1);
      const decoded = profile(serializationPrograms[0][1]);
      assert.equal(decoded.result.tag, "ok");
      assert.equal(decoded.stats.phase, 7);
      assert.equal(decoded.stats.renderedBytes, 0);
      assert.ok(decoded.stats.syntaxNodes > 0);
      assert.ok(decoded.stats.syntaxStorage > 360000);
      assert.ok(limited.stats.signatureComputations > 0);
      const badAlias = profile('(module (@type.alias $Bad $Bad) (func $entry (param $x $Bad)))');
      assert.equal(badAlias.result.tag, "err");
      assert.equal(badAlias.result.val.code, 206);
      assert.deepEqual(profile(repeated(10)), small, "failed requests do not poison later caches");
      const malformed = value.api.profileProject([{path: "main.walz", source: new Uint8Array([255])}]);
      assert.equal(malformed.result.tag, "err");
      assert.equal(malformed.result.val.code, 1);
      assert.equal(malformed.stats.phase, 1);
      assert.equal(malformed.stats.typeVisits, 0);
      clean();
      if (backend === "core") {
        const input = join(dir, 'input.walz'), output = join(dir, 'reports/profile.json');
        await writeFile(input, repeated(10));
        await exec('node', ['runtime/selfhost-profile.mjs', input, output, dir]);
        const captured = JSON.parse(await readFile(output));
        assert.equal(captured.schema, 'walz-selfhost-profile-v4');
        assert.equal(captured.countersSchema, 'walz-selfhost-stats-v4');
        assert.equal(captured.phase, 'complete');
        assert.ok(captured.cacheHitRates.signature > 0);
        assert.equal(captured.sources[0].sha256, createHash('sha256').update(repeated(10)).digest('hex'));
        await exec('node', ['runtime/selfhost-profile-compare.mjs', dir, dir, input, output, '3']);
        const compared = JSON.parse(await readFile(output));
        assert.equal(compared.binaryEquality, true);
        assert.equal(compared.timedSamples, 3);
        assert.ok(Number.isFinite(compared.speedup));
        await writeFile(input, badCall);
        await assert.rejects(exec('node', ['runtime/selfhost-profile.mjs', input, output, dir]), (error) => error.code === 1);
        assert.equal(JSON.parse(await readFile(output)).result.val.code, 110, 'failed requests still write a report');
        await exec('node', ['runtime/selfhost-profile-compare.mjs', dir, dir, input, output, '3']);
        const rejected = JSON.parse(await readFile(output));
        assert.equal(rejected.binaryEquality, false);
        assert.equal(rejected.speedup, null);
        assert.equal(rejected.timedSamples, 0, 'do not benchmark failed or incomparable outcomes');
      }
      await collect();
      clean();
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
