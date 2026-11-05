// Fixed stock-ledger oracle. Reader scores never influence execution checks.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { build } from "../../runtime/build-core.mjs";
import { instrument } from "../../runtime/testing/memory.mjs";
import { createTextIO, TEXT_IO, IOError } from "../../runtime/text-io.mjs";
const exec = promisify(rawExec);
const projectRoot = fileURLToPath(new URL("../../", import.meta.url));
export async function verifyStock({ source, output, checkInputs }) {
  const checks = [];
  const check = async (name, run) => {
    try {
      const evidence = await run();
      checks.push({ name, passed: true, evidence });
    } catch (error) {
      checks.push({ name, passed: false, error: error.message });
    }
  };
  await check("frozen inputs", checkInputs);
  if (!checks[0].passed) return { passed: false, checks };
  let runtime;
  await check("W1 type and Wasm validation", async () => {
    await build(source, output, { legacySyntax: true });
    await instrument(output, "core");
    runtime = await import(pathToFileURL(join(output, "index.js")));
  });
  await check("W4 pair state and one initialization", async () => {
    const { stdout } = await exec(
      "cargo",
      ["run", "--locked", "--quiet", "--", "--legacy-syntax", "ir", source],
      { cwd: projectRoot, maxBuffer: 8 * 1024 * 1024 },
    );
    const ir = JSON.parse(stdout),
      main = ir.functions.find((f) => f.name === "$main");
    const stateType = ir.types.findIndex((t) => t.name === "$stock.State");
    const pairType = ir.types.findIndex((t) => {
      const p = t.kind?.Value?.Pair;
      return (
        p?.first?.Ref?.ty === stateType &&
        p.first.Ref.nullable === false &&
        p.second?.Num === "i32"
      );
    });
    assert.notEqual(pairType, -1, "pair<ref State,i32> must exist");
    assert.ok(
      main.locals.some(
        (l) => l.kind?.Loop !== undefined && l.ty?.Value === pairType,
      ),
      "pair must be a fold loop binding",
    );
    const calls = [];
    const walk = (node, inLoopBody = false) => {
      if (!node || typeof node !== "object") return;
      if (
        node.kind?.Call &&
        ir.functions[node.kind.Call.function].name === "$stock.new"
      )
        calls.push({ inLoopBody, args: node.kind.Call.args });
      for (const [key, value] of Object.entries(node)) {
        if (key === "Loop" && value?.body) {
          walk(value.bindings, inLoopBody);
          walk(value.body, true);
        } else if (Array.isArray(value))
          value.forEach((v) => walk(v, inLoopBody));
        else walk(value, inLoopBody);
      }
    };
    walk(main.body);
    assert.equal(calls.length, 1, "one stock.new call in main");
    assert.equal(
      calls[0].inLoopBody,
      false,
      "initialization must be outside the loop body",
    );
    assert.equal(calls[0].args[0].kind.Const.literal, "4096");
    const text = await readFile(source, "utf8");
    assert.match(text, /\(%?stream\.fold-result\s/);
    assert.match(text, /\(%?result\.let\s/);
    return { pairType, stockNewCalls: calls.length, outsideLoopBody: true };
  });
  const encode = (s) => new TextEncoder().encode(s);
  const open = async (input, write) => {
    let arena;
    const program = await runtime.instantiate({
      imports: { [TEXT_IO]: createTextIO({ input, write }) },
      instantiateCore: async (m, i) => {
        const instance = await WebAssembly.instantiate(m, i);
        if (instance.exports.alloc) arena = instance.exports;
        return instance;
      },
    });
    return {
      ...program,
      clean() {
        assert.equal(arena.live(), 0);
        assert.ok(Object.values(program.__memory()).every((n) => n === 0));
      },
    };
  };
  if (runtime) {
    const add = (key, stock) => ({ op: "ADD", key, ok: true, quantity: stock });
    const show = (key, stock) => ({
      op: "SHOW",
      key,
      ok: true,
      quantity: stock,
    });
    const cases = [
      ["empty", "", 0, []],
      ["blank and comment", "\n# comment\n", 2, []],
      [
        "mixed",
        "ADD\ta\t1\nSHOW\ta\ninvalid\n",
        3,
        [add("a", 1), show("a", 1), { ok: false, error: "invalid-command" }],
      ],
      [
        "state reuse, no trailing LF",
        "ADD\ta\t1\nADD\ta\t2\nSHOW\ta",
        3,
        [add("a", 1), add("a", 3), show("a", 3)],
      ],
      [
        "business failure continues",
        "TAKE\tmissing\t1\nSHOW\tmissing\n",
        2,
        [
          {
            op: "TAKE",
            key: "missing",
            ok: false,
            error: "insufficient-stock",
          },
          show("missing", 0),
        ],
      ],
      [
        "unicode and CRLF",
        "ADD\t日本語🦀\t2\r\nSHOW\t日本語🦀\r\n",
        2,
        [add("日本語🦀", 2), show("日本語🦀", 2)],
      ],
    ];
    for (const [name, text, count, expected] of cases)
      await check(`W2 ${name}`, async () => {
        const lines = [];
        let closed = 0;
        const bytes = encode(text);
        const program = await open(
          (async function* () {
            try {
              for (let i = 0; i < bytes.length; i += 7)
                yield bytes.subarray(i, i + 7);
            } finally {
              closed++;
            }
          })(),
          async (b) => lines.push(JSON.parse(new TextDecoder().decode(b))),
        );
        const result = await program.api.main();
        // Check output and cleanup even when the returned count is wrong.
        assert.deepEqual(lines, expected);
        assert.equal(closed, 1);
        program.clean();
        assert.deepEqual(result, { tag: "ok", val: count });
        return { result, json: lines, closed, handles: 0, arenaLive: 0 };
      });
    for (const phase of ["utf8", "read", "write"])
      await check(`W3 ${phase} failure`, async () => {
        let closed = 0,
          writes = 0;
        const program = await open(
          (async function* () {
            try {
              if (phase === "read") throw new Error("read failure");
              yield phase === "utf8"
                ? new Uint8Array([255, 10])
                : encode("ADD\ta\t1\nSHOW\ta\n");
            } finally {
              closed++;
            }
          })(),
          async () => {
            writes++;
            throw new Error("write failure");
          },
        );
        const result = await program.api.main();
        assert.deepEqual(result, { tag: "err", val: IOError[phase] });
        assert.equal(writes, phase === "write" ? 1 : 0);
        assert.equal(closed, 1);
        program.clean();
        return { result, closed, writes, handles: 0, arenaLive: 0 };
      });
    for (const phase of ["read", "write"])
      await check(`W3 ${phase} cancellation`, async () => {
        const entered = Promise.withResolvers(),
          disposing = Promise.withResolvers(),
          release = Promise.withResolvers();
        let closed = 0;
        const input =
          phase === "read"
            ? {
                [Symbol.asyncIterator]() {
                  return this;
                },
                next() {
                  entered.resolve();
                  return new Promise(() => {});
                },
                async return() {
                  closed++;
                  disposing.resolve();
                  await release.promise;
                  return { done: true };
                },
              }
            : (async function* () {
                try {
                  yield encode("ADD\ta\t1\n");
                } finally {
                  closed++;
                }
              })();
        const program = await open(input, async (_, signal) => {
          entered.resolve();
          await new Promise((r) =>
            signal.addEventListener("abort", r, { once: true }),
          );
          disposing.resolve();
          await release.promise;
        });
        const task = program.start("main"),
          rejected = assert.rejects(task.result, {
            name: "TaskCancelledError",
          });
        await entered.promise;
        let acknowledged = false;
        const cancelled = task.cancel().then((r) => {
          acknowledged = true;
          return r;
        });
        await disposing.promise;
        assert.equal(acknowledged, false);
        release.resolve();
        assert.equal((await cancelled).status, "cancelled");
        await rejected;
        assert.equal(closed, 1);
        program.clean();
        return {
          closed,
          acknowledgedAfterCleanup: true,
          handles: 0,
          arenaLive: 0,
        };
      });
  }
  const result = { passed: checks.every((c) => c.passed), checks };
  return result;
}
