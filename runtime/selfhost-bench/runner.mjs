import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";

export function requireEqualOutput(actual, expected) {
  const view = bytes => ArrayBuffer.isView(bytes)
    ? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    : new Uint8Array(bytes);
  assert.deepEqual(view(actual), view(expected), "compiler output differs");
}

// Verify every result outside the clock. Clock overhead is retained in samples.
export async function measure(variant, iterations) {
  let ns = 0;
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    const result = variant.run();
    ns += (performance.now() - start) * 1e6;
    variant.verify(result);
  }
  return { calls: iterations, nsPerCall: ns / iterations };
}

export function verifyExecution(exports, probes) {
  for (const { name, args, expected, trap } of probes) {
    if (trap) assert.throws(() => exports[name](...args), WebAssembly.RuntimeError);
    else assert.equal(exports[name](...args), expected, `${name}(${args.join(",")})`);
  }
}
