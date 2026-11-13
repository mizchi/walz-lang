// Identical module-worker harness for every compiler. Excluded from payload totals.
self.onmessage = async ({ data: { backend, cases } }) => {
  try {
    let compile;
    if (backend === "rust" || backend === "wat") {
      const module = await import(backend === "rust" ? "/rust/pkg/walz_web.js" : "/wat/pkg/wat_web.js");
      await module.default();
      compile = source => {
        if (backend === "wat") return module.compile_source(source);
        const result = JSON.parse(module.compile_source(source, true));
        if (result.status !== "success") throw new Error(JSON.stringify(result.diagnostic));
        return new Uint8Array(result.wasm);
      };
    } else {
      const { instantiate } = await import(`/${backend}/index.js`);
      const { api } = await instantiate();
      compile = source => {
        const result = api.compile(new TextEncoder().encode(source));
        if (result.tag !== "ok") throw new Error(JSON.stringify(result.val));
        return result.val;
      };
    }
    const outputs = [];
    let probes = 0, traps = 0;
    for (const c of cases) {
      const wasm = compile(c.source);
      if (!WebAssembly.validate(wasm)) throw new Error(`${c.id}: invalid output`);
      const { instance } = await WebAssembly.instantiate(wasm);
      const values = JSON.parse(c.probes, (_, v) => v && typeof v === "object" && typeof v.i64 === "string" ? BigInt(v.i64) : v);
      for (const p of values) {
        if (p.trap) {
          let trapped = false;
          try { instance.exports[p.name](...p.args); }
          catch (error) { if (!(error instanceof WebAssembly.RuntimeError)) throw error; trapped = true; }
          if (!trapped) throw new Error(`${c.id}/${p.name}: expected trap`);
          traps++;
        } else {
          const actual = instance.exports[p.name](...p.args);
          if (actual !== p.expected) throw new Error(`${c.id}/${p.name}: ${actual} != ${p.expected}`);
        }
        probes++;
      }
      outputs.push({ id: c.id, wasm });
    }
    let rejected = false;
    try { compile('(module (func $bad (result i64) (i32.const 1)))'); }
    catch { rejected = true; }
    if (!rejected) throw new Error("ill-typed input was accepted");
    self.postMessage({ outputs, probes, traps, illTypedRejected: rejected }, outputs.map(o => o.wasm.buffer));
  } catch (error) { self.postMessage({ error: String(error.stack ?? error) }); }
};
